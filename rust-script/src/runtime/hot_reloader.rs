use std::{rc::Rc, sync::mpsc::TryRecvError};

use abi_stable::std_types::{RBox, Tuple2};
use godot::{
    engine::Engine,
    prelude::{
        godot_api, godot_error, godot_warn, Base, Callable, Gd, GodotClass, Object, RefCounted,
        StringName, Variant,
    },
};
use hot_lib_reloader::LibReloadObserver;

use crate::{
    apply::Apply,
    script_registry::{RemoteGodotScript_TO, RemoteValueRef},
    RustScriptLibInit,
};

use super::{rust_script::RustScript, HOT_RELOAD_BRIDGE};

enum Signal {
    Unload,
    Reload,
}

#[derive(GodotClass)]
#[base(RefCounted)]
pub struct HotReloader {
    channel: std::sync::mpsc::Receiver<Signal>,
    ffi_init_fn: Rc<dyn RustScriptLibInit>,
    base: Base<RefCounted>,
}

#[godot_api]
impl HotReloader {
    pub fn new(
        subscription: LibReloadObserver,
        ffi_init_fn: Rc<dyn RustScriptLibInit>,
        base: Base<RefCounted>,
    ) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();

        std::thread::spawn(move || loop {
            let block = subscription.wait_for_about_to_reload();

            if let Err(err) = sender.send(Signal::Unload) {
                godot_error!("Hot Reloader: {}", err);
                break;
            }

            drop(block);

            subscription.wait_for_reload();

            if let Err(err) = sender.send(Signal::Reload) {
                godot_error!("Hot Reloader: {}", err);
                break;
            }
        });

        Self {
            channel: receiver,
            ffi_init_fn,
            base,
        }
    }

    #[func]
    fn poll(&self) {
        let result = self.channel.try_recv();

        match result {
            Ok(Signal::Unload) => godot_warn!("about to hot reload rust scripts!"),
            Ok(Signal::Reload) => {
                godot_warn!("reloading rust scripts...");

                super::load_rust_scripts(self.ffi_init_fn.clone());

                HOT_RELOAD_BRIDGE.with(|bridge| {
                    let instances = bridge.borrow();

                    instances.values().for_each(|cell| {
                        let mut entry = cell.borrow_mut();

                        let old_state = entry.instance.property_state();

                        let new_instance = entry
                            .script
                            .bind()
                            .create_remote_instance(entry.base.clone())
                            .apply(move |inst| {
                                old_state.into_iter().for_each(|Tuple2(prop, value)| {
                                    let variant: Variant = value.into();

                                    inst.set(prop, RemoteValueRef::new(&variant));
                                })
                            });

                        entry.instance = new_instance;
                    })
                });
            }
            Err(TryRecvError::Disconnected) => {
                godot_error!("hot reloader thread got disconnected!");
            }

            Err(TryRecvError::Empty) => (),
        }
    }

    #[func]
    fn register(&self) {
        Engine::singleton()
            .get_main_loop()
            .expect("we have to have a main loop")
            .connect(
                StringName::from("process_frame"),
                Callable::from_object_method(self.base.clone(), "poll"),
            );
    }
}

pub(super) struct HotReloadEntry {
    pub instance: RemoteGodotScript_TO<'static, RBox<()>>,
    script: Gd<RustScript>,
    base: Gd<Object>,
}

impl HotReloadEntry {
    pub fn new(
        instance: RemoteGodotScript_TO<'static, RBox<()>>,
        script: Gd<RustScript>,
        base: Gd<Object>,
    ) -> Self {
        Self {
            instance,
            script,
            base,
        }
    }
}
