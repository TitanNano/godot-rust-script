/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use godot::{
    builtin::{GString, PackedStringArray},
    classes::{
        EditorExportPlugin, EditorPlugin, IEditorExportPlugin, IEditorPlugin, Node, Resource,
    },
    obj::{Base, Gd, NewGd, WithBaseField},
    prelude::{godot_api, GodotClass},
};

#[derive(GodotClass)]
#[class(base = EditorPlugin, tool )]
pub struct RustScriptEditorPlugin {
    base: Base<EditorPlugin>,
    export_plugin: Gd<RustScriptExportPlugin>,
}

#[godot_api]
impl IEditorPlugin for RustScriptEditorPlugin {
    fn init(base: Base<Self::Base>) -> Self {
        Self {
            base,
            export_plugin: RustScriptExportPlugin::new_gd(),
        }
    }
    fn enter_tree(&mut self) {
        let export_plugin = self.export_plugin.clone();

        self.base_mut().add_export_plugin(&export_plugin);
    }

    fn exit_tree(&mut self) {
        let export_plugin = self.export_plugin.clone();

        self.base_mut().remove_export_plugin(&export_plugin);
    }
}

#[derive(GodotClass)]
#[class(base = EditorExportPlugin, tool, init)]
struct RustScriptExportPlugin {
    base: Base<EditorExportPlugin>,
}

#[godot_api]
impl IEditorExportPlugin for RustScriptExportPlugin {
    #[expect(unused_variables)]
    fn customize_resource(
        &mut self,
        resource: godot::prelude::Gd<Resource>,
        path: godot::prelude::GString,
    ) -> Option<godot::prelude::Gd<Resource>> {
        None
    }

    #[expect(unused_variables)]
    fn customize_scene(
        &mut self,
        scene: godot::prelude::Gd<Node>,
        path: godot::prelude::GString,
    ) -> Option<godot::prelude::Gd<Node>> {
        None
    }

    fn get_customization_configuration_hash(&self) -> u64 {
        0
    }

    fn get_name(&self) -> godot::prelude::GString {
        GString::from("RustScriptExportPlugin")
    }

    fn export_file(&mut self, path: GString, _type_: GString, _features: PackedStringArray) {
        if !path.ends_with(".rs") {
            return;
        }

        self.base_mut().skip();
    }
}
