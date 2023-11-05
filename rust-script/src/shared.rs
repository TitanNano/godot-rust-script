use abi_stable::std_types::RVec;

use crate::RemoteScriptMetaData;

pub type BindingInit = godot::sys::GodotBinding;

pub trait RustScriptLibInit: Fn(Option<BindingInit>) -> RVec<RemoteScriptMetaData> {}

impl<F> RustScriptLibInit for F where F: Fn(Option<BindingInit>) -> RVec<RemoteScriptMetaData> {}
