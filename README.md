# Godot Rust Script
An implementation of the rust programing language as a scripting language for the godot engine based on [godot-rust/gdext](https://github.com/godot-rust/gdext).

# Important Notice

**godot-rust-script is still very experimental and unstable.**

This project also currently depends on a slightly modfied fork of [godot-rust/gdext](https://github.com/godot-rust/gdext) and should not be used in combination with any other version.

# Featues
- use rust as scripts similar to GDScript or CSharp
- hot reload your rust scripts in development mode
- use familiar godot annotations similar to GDScripts annotations for editor integration

# Setup

godot-rust-script comes with two compontents. A script runtime and a library for writing godot script compatible rust structs. Both these components have to be set up.

Two sepearte crates are required to make rust scripts work.

## Runtime library

rs-runtime/Cargo.toml

```toml
[package]
name = "rs-runtime"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
godot-rust-script = { git  = "https://github.com/TitanNano/godot-rust-script.git", branch = "master" }
scripts = { path = "./scripts" }
```

rs-runtime/src/main.rs

```rs
use std::cell::RefCell;

use godot::prelude::{gdextension, ExtensionLibrary, InitLevel};
use godot_rust_script::{self, RustScriptExtensionLayer};

godot_rust_script::setup!(scripts);

struct NativeLib;

thread_local! {
    static RUST_SCRIPT_LAYER: RefCell<RustScriptExtensionLayer> = RefCell::new(godot_rust_script::init!());
}

#[gdextension]
unsafe impl ExtensionLibrary for NativeLib {
    fn on_level_init(level: InitLevel) {
        match level {
            InitLevel::Core => (),
            InitLevel::Servers => (),
            InitLevel::Scene => RUST_SCRIPT_LAYER.with_borrow_mut(|layer| layer.initialize()),
            InitLevel::Editor => (),
        }
    }

    fn on_level_deinit(level: InitLevel) {
        match level {
            InitLevel::Editor => (),
            InitLevel::Scene => RUST_SCRIPT_LAYER.with_borrow_mut(|layer| layer.deinitialize()),
            InitLevel::Servers => {}
            InitLevel::Core => (),
        }
    }
}
```

## Scripts Library

scripts-lib/Cargo.toml

```toml
[package]
name = "scripts"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["dylib", "rlib"]

[dependencies]
godot-rust-script = { git  = "https://github.com/TitanNano/godot-rust-script.git", branch = "master" }
```

scripts-lib/src/lib.rs

```rs
mod example_script;

godot_rust_script::setup_library!();
```

scripts-lib/src/example_script.rs

```rs
use godot_rust_script::{
	godot::prelude::{godot_print, Gd, GodotString, Node3D, Object},
	godot_script_impl, GodotScript,
};

#[derive(Debug, GodotScript)]
struct ExampleScript {
	#[export(exp_easing = ["positive_only"])]
	pub flag: bool,
	//#[export(dir_path, color_no_alpha, global_dir, global_file, multiline)]
	pub path: GodotString,
	property: Option<Gd<Object>>,
	base: Gd<Node3D>,
}

#[godot_script_impl]
impl ExampleScript {
	pub fn perform_action(&self, value: i32) -> bool {
		value > 0
	}

	pub fn _process(&mut self, delta: f64) {
		godot_print!(
			"example script doing process stuff: {}, {}",
			delta,
			self.base
		);
	}
}
```
