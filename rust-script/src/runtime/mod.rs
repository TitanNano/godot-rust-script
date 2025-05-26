/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

mod call_context;
mod downgrade_self;
mod editor;
mod metadata;
mod resource_loader;
mod resource_saver;
mod rust_script;
mod rust_script_instance;
mod rust_script_language;

use std::{collections::HashMap, sync::RwLock};

use godot::classes::{
    Engine, RefCounted, ResourceFormatLoader, ResourceFormatSaver, ResourceLoader, ResourceSaver,
    ScriptLanguage,
};
use godot::global::godot_warn;
use godot::obj::{GodotClass, Inherits};
use godot::prelude::{godot_print, Gd};
use godot::register::GodotClass;
use once_cell::sync::Lazy;

use crate::runtime::{
    resource_loader::RustScriptResourceLoader, resource_saver::RustScriptResourceSaver,
};
use crate::static_script_registry::RustScriptMetaData;

use self::rust_script_language::RustScriptLanguage;

pub use call_context::Context;
pub(crate) use rust_script::RustScript;
pub(crate) use rust_script_instance::GodotScriptObject;

static SCRIPT_REGISTRY: Lazy<RwLock<HashMap<String, RustScriptMetaData>>> =
    Lazy::new(RwLock::default);

#[derive(GodotClass)]
#[class(base = Object, init)]
struct RefCountedSingleton {
    inner: Gd<RefCounted>,
}

impl RefCountedSingleton {
    fn new<T: Inherits<RefCounted>>(object: &Gd<T>) -> Gd<Self> {
        Gd::from_object(Self {
            inner: object.clone().upcast(),
        })
    }

    fn get(&self) -> Gd<RefCounted> {
        self.inner.clone()
    }
}

pub trait RustScriptLibInit: Fn() -> Vec<RustScriptMetaData> {}

impl<F> RustScriptLibInit for F where F: Fn() -> Vec<RustScriptMetaData> {}

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

        engine.register_script_language(&lang);
        engine.register_singleton(&RustScriptLanguage::class_name().to_string_name(), &lang);

        ResourceSaver::singleton().add_resource_format_saver(&res_saver);
        engine.register_singleton(
            &RustScriptResourceSaver::class_name().to_string_name(),
            &RefCountedSingleton::new(&res_saver),
        );

        ResourceLoader::singleton().add_resource_format_loader(&res_loader);
        engine.register_singleton(
            &RustScriptResourceLoader::class_name().to_string_name(),
            &RefCountedSingleton::new(&res_loader),
        );

        godot_print!("finished registering rust scripting language!");
    }

    pub fn deinitialize() {
        godot_print!("deregistering rust scripting language...");
        let mut engine = Engine::singleton();

        if let Some(lang) = engine
            .get_singleton(&RustScriptLanguage::class_name().to_string_name())
            .map(Gd::cast::<ScriptLanguage>)
        {
            engine.unregister_script_language(&lang);
            engine.unregister_singleton(&RustScriptLanguage::class_name().to_string_name());
            lang.free();
        }

        if let Some(res_loader_singleton) = engine
            .get_singleton(&RustScriptResourceLoader::class_name().to_string_name())
            .map(Gd::cast::<RefCountedSingleton>)
        {
            let res_loader = res_loader_singleton.bind().get();

            if res_loader.get_reference_count() != 3 {
                godot_warn!(
                    "RustScriptResourceLoader's ref count is off! {} but expected 3",
                    res_loader.get_reference_count()
                );
            }

            ResourceLoader::singleton()
                .remove_resource_format_loader(&res_loader.cast::<ResourceFormatLoader>());
            engine.unregister_singleton(&RustScriptResourceLoader::class_name().to_string_name());
            res_loader_singleton.free();
        }

        if let Some(res_saver_singleton) = engine
            .get_singleton(&RustScriptResourceSaver::class_name().to_string_name())
            .map(Gd::cast::<RefCountedSingleton>)
        {
            let res_saver = res_saver_singleton.bind().get();

            if res_saver.get_reference_count() != 3 {
                godot_warn!(
                    "RustScriptResourceSaver's ref count is off! {} but expected 3",
                    res_saver.get_reference_count()
                );
            }

            ResourceSaver::singleton()
                .remove_resource_format_saver(&res_saver.clone().cast::<ResourceFormatSaver>());
            engine.unregister_singleton(&RustScriptResourceSaver::class_name().to_string_name());
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
