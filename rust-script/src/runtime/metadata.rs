use std::rc::Rc;

use abi_stable::std_types::RBox;
use godot::{
    obj::EngineEnum,
    prelude::{
        meta::{ClassName, MethodInfo, PropertyInfo},
        Array, Dictionary, Gd, Object, StringName, ToGodot,
    },
};

use crate::{
    apply::Apply,
    script_registry::{CreateScriptInstanceData_TO, RemoteGodotScript_TO, RemoteScriptMetaData},
};

#[derive(Debug)]
pub struct ScriptMetaData {
    class_name: ClassName,
    base_type_name: StringName,
    properties: Rc<Vec<PropertyInfo>>,
    methods: Rc<Vec<MethodInfo>>,
    create_data: CreateScriptInstanceData_TO<'static, RBox<()>>,
}

impl ScriptMetaData {
    pub fn class_name(&self) -> ClassName {
        self.class_name
    }

    pub fn base_type_name(&self) -> StringName {
        self.base_type_name.clone()
    }

    pub fn create_data(&self, base: Gd<Object>) -> RemoteGodotScript_TO<'static, RBox<()>> {
        self.create_data.create(base.to_variant().into())
    }

    pub fn properties(&self) -> Rc<Vec<PropertyInfo>> {
        self.properties.clone()
    }

    pub fn methods(&self) -> Rc<Vec<MethodInfo>> {
        self.methods.clone()
    }
}

impl From<RemoteScriptMetaData> for ScriptMetaData {
    fn from(value: RemoteScriptMetaData) -> Self {
        Self {
            class_name: ClassName::from_ascii_cstr(value.class_name.as_str().as_bytes()),
            base_type_name: StringName::from(value.base_type_name.as_str()),
            properties: Rc::new(value.properties.into_iter().map(Into::into).collect()),
            methods: Rc::new(value.methods.into_iter().map(Into::into).collect()),
            create_data: value.create_data,
        }
    }
}

pub(super) trait ToDictionary {
    fn to_dict(&self) -> Dictionary;
}

impl ToDictionary for PropertyInfo {
    fn to_dict(&self) -> Dictionary {
        let mut dict = Dictionary::new();

        dict.set("name", self.property_name.clone());
        dict.set("class_name", self.class_name.to_string_name());
        dict.set("type", self.variant_type as i32);
        dict.set("hint", self.hint.ord());
        dict.set("hint_string", self.hint_string.clone());
        dict.set("usage", self.usage.ord());

        dict
    }
}

impl ToDictionary for MethodInfo {
    fn to_dict(&self) -> Dictionary {
        Dictionary::new().apply(|dict| {
            dict.set("name", self.method_name.clone());
            dict.set("flags", self.flags.ord());

            let args: Array<_> = self.arguments.iter().map(|arg| arg.to_dict()).collect();

            dict.set("args", args);

            dict.set("return", self.return_type.to_dict());
        })
    }
}
