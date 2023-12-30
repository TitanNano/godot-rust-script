use abi_stable::std_types::RVec;

use crate::script_registry::RemoteScriptMetaData;

pub trait RustScriptLibInit: Fn() -> RVec<RemoteScriptMetaData> {}

impl<F> RustScriptLibInit for F where F: Fn() -> RVec<RemoteScriptMetaData> {}
