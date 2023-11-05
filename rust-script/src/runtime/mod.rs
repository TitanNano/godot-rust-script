#[cfg(all(feature = "hot-reload", debug_assertions))]
mod hot_reloader;
mod metadata;
mod resource_loader;
mod resource_saver;
mod rust_script;
mod rust_script_instance;
mod rust_script_language;

use std::{collections::HashMap, rc::Rc, sync::RwLock};

use cfg_if::cfg_if;
use godot::{
    engine::{Engine, ResourceLoader, ResourceSaver},
    prelude::{godot_print, Gd},
};

use crate::{
    runtime::{
        metadata::ScriptMetaData, resource_loader::RustScriptResourceLoader,
        resource_saver::RustScriptResourceSaver,
    },
    shared::RustScriptLibInit,
};

use self::rust_script_language::RustScriptLanguage;

#[cfg(all(feature = "hot-reload", debug_assertions))]
use hot_reloader::{HotReloadEntry, HotReloader};

#[cfg(all(feature = "hot-reload", debug_assertions))]
#[macro_export]
macro_rules! setup {
    ($lib_crate:tt) => {
        #[$crate::private_export::hot_module(dylib = stringify!($lib_crate), lib_dir=process_path::get_dylib_path().and_then(|path| path.parent().map(std::path::Path::to_path_buf)).unwrap_or_default())]
        mod scripts_lib {
            use $crate::private_export::RVec;

            // manually expose functions.
            #[hot_functions]
            extern "Rust" {
                pub fn __godot_rust_script_init(
                    binding: Option<$crate::private_export::BindingInit>,
                ) -> RVec<$crate::RemoteScriptMetaData>;
            }

            // expose a type to subscribe to lib load events
            #[lib_change_subscription]
            pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}

            pub use ::$lib_crate::__GODOT_RUST_SCRIPT_SRC_ROOT;
        }
    };
}
    
#[cfg(not(all(feature = "hot-reload", debug_assertions)))]
#[macro_export]
macro_rules! setup {
    ($lib_crate:tt) => {
        mod scripts_lib {
            pub use ::$lib_crate::{__godot_rust_script_init, __GODOT_RUST_SCRIPT_SRC_ROOT};
        }
    };
}

#[cfg(not(all(feature = "hot-reload", debug_assertions)))]
#[macro_export]
macro_rules! init {
    () => {
        $crate::RustScriptExtensionLayer::new(
            scripts_lib::__godot_rust_script_init,
            scripts_lib::__GODOT_RUST_SCRIPT_SRC_ROOT,
        )
    };
}

#[cfg(all(feature = "hot-reload", debug_assertions))]
#[macro_export]
macro_rules! init {
    () => {
        $crate::RustScriptExtensionLayer::new(
            scripts_lib::__godot_rust_script_init,
            scripts_lib::__GODOT_RUST_SCRIPT_SRC_ROOT,
            scripts_lib::subscribe,
        )
    };
}

thread_local! {
    static SCRIPT_REGISTRY: RwLock<HashMap<String, ScriptMetaData>> = RwLock::default();
    #[cfg(all(feature = "hot-reload", debug_assertions))]
    static HOT_RELOAD_BRIDGE: std::cell::RefCell<HashMap<rust_script_instance::RustScriptInstanceId, std::cell::RefCell<HotReloadEntry>>> = std::cell::RefCell::default();
}

cfg_if! {
    if #[cfg(all(feature = "hot-reload", debug_assertions))] {
        type HotReloadSubscribe = fn() -> hot_lib_reloader::LibReloadObserver;
    }
}

pub struct RustScriptExtensionLayer {
    lib_init_fn: ::std::rc::Rc<dyn RustScriptLibInit>,
    lang: Option<Gd<RustScriptLanguage>>,
    res_saver: Option<Gd<RustScriptResourceSaver>>,
    res_loader: Option<Gd<RustScriptResourceLoader>>,
    scripts_src_dir: Option<&'static str>,

    #[cfg(all(feature = "hot-reload", debug_assertions))]
    hot_reload_subscribe: HotReloadSubscribe,

    #[cfg(all(feature = "hot-reload", debug_assertions))]
    hot_reloader: Option<Gd<HotReloader>>,
}

impl RustScriptExtensionLayer {
    pub fn new<F: RustScriptLibInit + 'static + Clone>(
        lib_init_fn: F,
        scripts_src_dir: &'static str,
        #[cfg(all(feature = "hot-reload", debug_assertions))]
        hot_reload_subscribe: HotReloadSubscribe,
    ) -> Self {
        Self {
            lib_init_fn: Rc::new(lib_init_fn),
            lang: None,
            res_saver: None,
            res_loader: None,
            scripts_src_dir: Some(scripts_src_dir),

            #[cfg(all(feature = "hot-reload", debug_assertions))]
            hot_reload_subscribe,

            #[cfg(all(feature = "hot-reload", debug_assertions))]
            hot_reloader: None,
        }
    }

    pub fn initialize(&mut self) {
        godot_print!("registering rust scripting language...");

        let lang: Gd<RustScriptLanguage> = RustScriptLanguage::new(self.scripts_src_dir);
        let res_loader = RustScriptResourceLoader::new(lang.clone());
        let res_saver = Gd::new(RustScriptResourceSaver);

        cfg_if! {
            if #[cfg(all(feature = "hot-reload", debug_assertions))] {
                use godot::prelude::StringName;

                let mut hot_reloader = Gd::with_base(|base| HotReloader::new((self.hot_reload_subscribe)(), self.lib_init_fn.clone(), base));

                hot_reloader.call_deferred(StringName::from("register"), &[]);

                self.hot_reloader = Some(hot_reloader);
            }
        }

        self.lang = Some(lang.clone());
        self.res_saver = Some(res_saver.clone());
        self.res_loader = Some(res_loader.clone());

        load_rust_scripts(self.lib_init_fn.clone());

        Engine::singleton().register_script_language(lang.upcast());
        ResourceSaver::singleton().add_resource_format_saver(res_saver.upcast());
        ResourceLoader::singleton().add_resource_format_loader(res_loader.upcast());

        godot_print!("finished registering rust scripting language!");
    }

    pub fn deinitialize(&mut self) {
        godot_print!("deregistering rust scripting language...");

        if let Some(lang) = self.lang.take() {
            Engine::singleton().unregister_script_language(lang.clone().upcast());
        }

        if let Some(res_loader) = self.res_loader.take() {
            ResourceLoader::singleton().remove_resource_format_loader(res_loader.clone().upcast());
        }

        if let Some(res_saver) = self.res_saver.take() {
            ResourceSaver::singleton().remove_resource_format_saver(res_saver.clone().upcast());
        }

        godot_print!("finished deregistering rust scripting language!");
    }
}

fn load_rust_scripts(lib_init_fn: Rc<dyn RustScriptLibInit>) {
    cfg_if! {
        if #[cfg(all(feature = "hot-reload", debug_assertions))] {
            let ffi_init = Some(unsafe { godot::sys::get_binding() });
        } else {
            let ffi_init = None;
        }
    }

    let result = lib_init_fn(ffi_init);

    let registry: HashMap<String, ScriptMetaData> = result
        .into_iter()
        .map(|script| {
            let local: ScriptMetaData = script.into();

            (local.class_name().to_string(), local)
        })
        .collect();

    SCRIPT_REGISTRY.with(|reg_lock| {
        let mut reg = reg_lock
            .write()
            .expect("script registry rw lock is poisoned");

        *reg = registry;
    });
}
