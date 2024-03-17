/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

mod downgrade_self;
mod metadata;
mod resource_loader;
mod resource_saver;
mod rust_script;
mod rust_script_instance;
mod rust_script_language;

use std::{collections::HashMap, sync::RwLock};

use godot::{
    engine::{Engine, RefCounted, ResourceLoader, ResourceSaver, ScriptLanguage},
    log::godot_warn,
    obj::GodotClass,
    prelude::{godot_print, Gd},
    register::GodotClass,
};
use once_cell::sync::Lazy;

use crate::{
    runtime::{resource_loader::RustScriptResourceLoader, resource_saver::RustScriptResourceSaver},
    shared::RustScriptLibInit,
    RustScriptMetaData,
};

use self::rust_script_language::RustScriptLanguage;

#[macro_export]
macro_rules! setup {
    ($lib_crate:tt) => {
        mod scripts_lib {
            pub use $lib_crate::{__godot_rust_script_init, __GODOT_RUST_SCRIPT_SRC_ROOT};
        }
    };
}

#[macro_export]
macro_rules! init {
    ($scripts_module:tt) => {
        $crate::RustScriptExtensionLayer::initialize(
            $scripts_module::__godot_rust_script_init,
            $scripts_module::__GODOT_RUST_SCRIPT_SRC_ROOT,
        )
    };
}

#[macro_export]
macro_rules! deinit {
    () => {
        $crate::RustScriptExtensionLayer::deinitialize()
    };
}

static SCRIPT_REGISTRY: Lazy<RwLock<HashMap<String, RustScriptMetaData>>> =
    Lazy::new(RwLock::default);

#[derive(GodotClass)]
#[class(base = Object, init)]
struct RefCountedSingleton {
    inner: Gd<RefCounted>,
}

impl RefCountedSingleton {
    fn new(object: Gd<RefCounted>) -> Gd<Self> {
        Gd::from_object(Self { inner: object })
    }

    fn get(&self) -> Gd<RefCounted> {
        self.inner.clone()
    }
}

pub struct RustScriptExtensionLayer;

impl RustScriptExtensionLayer {
    pub fn initialize<F: RustScriptLibInit + 'static + Clone>(
        lib_init_fn: F,
        scripts_src_dir: &'static str,
    ) {
        godot_print!("registering rust scripting language...");

        let lang: Gd<RustScriptLanguage> = RustScriptLanguage::new(Some(scripts_src_dir));
        let res_loader = RustScriptResourceLoader::new(lang.clone());
        let res_saver = Gd::from_object(RustScriptResourceSaver);

        let mut engine = Engine::singleton();

        godot_print!("loading rust scripts...");
        load_rust_scripts(lib_init_fn);

        engine.register_script_language(lang.clone().upcast());
        engine.register_singleton(
            RustScriptLanguage::class_name().to_string_name(),
            lang.upcast(),
        );

        ResourceSaver::singleton().add_resource_format_saver(res_saver.clone().upcast());
        engine.register_singleton(
            RustScriptResourceSaver::class_name().to_string_name(),
            RefCountedSingleton::new(res_saver.clone().upcast()).upcast(),
        );

        ResourceLoader::singleton().add_resource_format_loader(res_loader.clone().upcast());
        engine.register_singleton(
            RustScriptResourceLoader::class_name().to_string_name(),
            RefCountedSingleton::new(res_loader.upcast()).upcast(),
        );

        godot_print!("finished registering rust scripting language!");
    }

    pub fn deinitialize() {
        godot_print!("deregistering rust scripting language...");
        let mut engine = Engine::singleton();

        if let Some(lang) = engine
            .get_singleton(RustScriptLanguage::class_name().to_string_name())
            .map(Gd::cast::<ScriptLanguage>)
        {
            engine.unregister_script_language(lang.clone());
            engine.unregister_singleton(RustScriptLanguage::class_name().to_string_name());
            lang.free();
        }

        if let Some(res_loader_singleton) = engine
            .get_singleton(RustScriptResourceLoader::class_name().to_string_name())
            .map(Gd::cast::<RefCountedSingleton>)
        {
            let res_loader = res_loader_singleton.bind().get();

            if res_loader.get_reference_count() != 3 {
                godot_warn!(
                    "RustScriptResourceLoader's ref count is off! {} but expected 3",
                    res_loader.get_reference_count()
                );
            }

            ResourceLoader::singleton().remove_resource_format_loader(res_loader.cast());
            engine.unregister_singleton(RustScriptResourceLoader::class_name().to_string_name());
            res_loader_singleton.free();
        }

        if let Some(res_saver_singleton) = engine
            .get_singleton(RustScriptResourceSaver::class_name().to_string_name())
            .map(Gd::cast::<RefCountedSingleton>)
        {
            let res_saver = res_saver_singleton.bind().get();

            if res_saver.get_reference_count() != 3 {
                godot_warn!(
                    "RustScriptResourceSaver's ref count is off! {} but expected 3",
                    res_saver.get_reference_count()
                );
            }

            ResourceSaver::singleton().remove_resource_format_saver(res_saver.clone().cast());
            engine.unregister_singleton(RustScriptResourceSaver::class_name().to_string_name());
            res_saver_singleton.free();
        }

        godot_print!("finished deregistering rust scripting language!");
    }
}

fn load_rust_scripts<F: RustScriptLibInit>(lib_init_fn: F) {
    let result = lib_init_fn();

    let registry: HashMap<String, RustScriptMetaData> = result
        .into_iter()
        .map(|script| (script.class_name().to_string(), script))
        .collect();

    let mut reg = SCRIPT_REGISTRY
        .write()
        .expect("script registry rw lock is poisoned");

    *reg = registry;
}
