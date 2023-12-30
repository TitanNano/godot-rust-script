mod metadata;
mod resource_loader;
mod resource_saver;
mod rust_script;
mod rust_script_instance;
mod rust_script_language;

use std::{collections::HashMap, rc::Rc, sync::RwLock};

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

#[macro_export]
macro_rules! setup {
    ($lib_crate:tt) => {
        mod scripts_lib {
            pub use ::$lib_crate::{__godot_rust_script_init, __GODOT_RUST_SCRIPT_SRC_ROOT};
        }
    };
}

#[macro_export]
macro_rules! init {
    () => {
        $crate::RustScriptExtensionLayer::new(
            scripts_lib::__godot_rust_script_init,
            scripts_lib::__GODOT_RUST_SCRIPT_SRC_ROOT,
        )
    };
}

thread_local! {
    static SCRIPT_REGISTRY: RwLock<HashMap<String, ScriptMetaData>> = RwLock::default();
}

pub struct RustScriptExtensionLayer {
    lib_init_fn: ::std::rc::Rc<dyn RustScriptLibInit>,
    lang: Option<Gd<RustScriptLanguage>>,
    res_saver: Option<Gd<RustScriptResourceSaver>>,
    res_loader: Option<Gd<RustScriptResourceLoader>>,
    scripts_src_dir: Option<&'static str>,
}

impl RustScriptExtensionLayer {
    pub fn new<F: RustScriptLibInit + 'static + Clone>(
        lib_init_fn: F,
        scripts_src_dir: &'static str,
    ) -> Self {
        Self {
            lib_init_fn: Rc::new(lib_init_fn),
            lang: None,
            res_saver: None,
            res_loader: None,
            scripts_src_dir: Some(scripts_src_dir),
        }
    }

    pub fn initialize(&mut self) {
        godot_print!("registering rust scripting language...");

        let lang: Gd<RustScriptLanguage> = RustScriptLanguage::new(self.scripts_src_dir);
        let res_loader = RustScriptResourceLoader::new(lang.clone());
        let res_saver = Gd::from_object(RustScriptResourceSaver);

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
    let result = lib_init_fn();

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
