mod metadata;
mod resource_loader;
mod resource_saver;
mod rust_script;
mod rust_script_instance;
mod rust_script_language;

use std::{
    collections::HashMap,
    mem::ManuallyDrop,
    ops::Deref,
    sync::{Arc, RwLock},
};

use godot::{
    engine::{Engine, ResourceFormatLoader, ResourceFormatSaver, ResourceLoader, ResourceSaver},
    log::godot_warn,
    obj::GodotClass,
    prelude::{godot_print, Gd},
};
use once_cell::sync::Lazy;

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
        $crate::RustScriptExtensionLayer::initialize(
            scripts_lib::__godot_rust_script_init,
            scripts_lib::__GODOT_RUST_SCRIPT_SRC_ROOT,
        )
    };
}

#[macro_export]
macro_rules! deinit {
    () => {
        $crate::RustScriptExtensionLayer::deinitialize()
    };
}

static SCRIPT_REGISTRY: Lazy<RwLock<HashMap<String, ScriptMetaData>>> =
    Lazy::new(|| RwLock::default());
pub struct RustScriptExtensionLayer {}

impl RustScriptExtensionLayer {
    pub fn initialize<F: RustScriptLibInit + 'static + Clone>(
        lib_init_fn: F,
        scripts_src_dir: &'static str,
    ) {
        godot_print!("registering rust scripting language...");

        let lang: Gd<RustScriptLanguage> = RustScriptLanguage::new(Some(scripts_src_dir));
        let res_loader = ManuallyDrop::new(RustScriptResourceLoader::new(lang.clone()));
        let res_saver = ManuallyDrop::new(Gd::from_object(RustScriptResourceSaver));

        let mut engine = Engine::singleton();

        godot_print!("loading rust scripts...");
        load_rust_scripts(Arc::new(lib_init_fn));

        engine.register_script_language(lang.clone().upcast());
        engine.register_singleton(
            RustScriptLanguage::class_name().to_string_name(),
            lang.upcast(),
        );

        ResourceSaver::singleton().add_resource_format_saver(res_saver.deref().clone().upcast());
        engine.register_singleton(
            RustScriptResourceSaver::class_name().to_string_name(),
            res_saver.deref().clone().upcast(),
        );

        ResourceLoader::singleton().add_resource_format_loader(res_loader.deref().clone().upcast());
        engine.register_singleton(
            RustScriptResourceLoader::class_name().to_string_name(),
            res_loader.deref().clone().upcast(),
        );

        godot_print!("finished registering rust scripting language!");
    }

    pub fn deinitialize() {
        godot_print!("deregistering rust scripting language...");
        let mut engine = Engine::singleton();

        if let Some(lang) = engine
            .get_singleton(RustScriptLanguage::class_name().to_string_name())
            .map(Gd::cast)
        {
            engine.unregister_script_language(lang);
            engine.unregister_singleton(RustScriptLanguage::class_name().to_string_name());
        }

        if let Some(res_loader) = engine
            .get_singleton(RustScriptResourceLoader::class_name().to_string_name())
            .map(Gd::cast::<ResourceFormatLoader>)
        {
            let res_loader = ManuallyDrop::new(res_loader);

            if res_loader.get_reference_count() != 3 {
                godot_warn!(
                    "RustScriptResourceLoader's ref count is off! {} but expected 3",
                    res_loader.get_reference_count()
                );
            }

            ResourceLoader::singleton().remove_resource_format_loader(res_loader.deref().clone());
            engine.unregister_singleton(RustScriptResourceLoader::class_name().to_string_name());
        }

        if let Some(res_saver) = engine
            .get_singleton(RustScriptResourceSaver::class_name().to_string_name())
            .map(Gd::cast::<ResourceFormatSaver>)
        {
            let res_saver = ManuallyDrop::new(res_saver);

            if res_saver.get_reference_count() != 3 {
                godot_warn!(
                    "RustScriptResourceSaver's ref count is off! {} but expected 3",
                    res_saver.get_reference_count()
                );
            }

            ResourceSaver::singleton().remove_resource_format_saver(res_saver.deref().clone());
            engine.unregister_singleton(RustScriptResourceSaver::class_name().to_string_name());
        }

        godot_print!("finished deregistering rust scripting language!");
    }
}

fn load_rust_scripts(lib_init_fn: Arc<dyn RustScriptLibInit>) {
    let result = lib_init_fn();

    let registry: HashMap<String, ScriptMetaData> = result
        .into_iter()
        .map(|script| {
            let local: ScriptMetaData = script.into();

            (local.class_name().to_string(), local)
        })
        .collect();

    let mut reg = SCRIPT_REGISTRY
        .write()
        .expect("script registry rw lock is poisoned");

    *reg = registry;
}
