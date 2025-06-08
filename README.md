# Godot Rust Script
An implementation of the rust programing language as a scripting language for the godot 4.x engine based on [godot-rust/gdext](https://github.com/godot-rust/gdext).

# Important Notice

**godot-rust-script is still experimental and undergoes breaking changes from time to time.**

# Why?

The question of why this project exists might arise, and it's a good question. The [godot-rust/gdext](https://github.com/godot-rust/gdext)
project already implements excellent bindings with the engine and provides a good developer experience. If you are just looking to write code 
for your Godot project in rust, you most likely are already well served with gdext and definitely do not **need** this library.

## When would you want to use `godot-rust-script`?

GDExtension works by allowing dynamic libraries to define their own Godot classes, which inherit directly from an existing class. These 
classes inherit all the functionality of their base classes. Nothing more, nothing less. Scripts, on the other hand, offer a bit more 
flexibility. While they also define a base class, this is more like a minimally required interface. Scripts are attached to an instance of 
an existing class. As long as the instance inherits the required base class, the script is compatible with it. This makes the scripts somewhat 
more flexible and provides more compossibility than using plain class-based inheritance. It is up to you to decide if you need this
additional flexibility.

# Setup

To use `godot-rust-script` first follow the basic setup instructions for `gdext`.

## Add Dependency

The project has to be added as a cargo dependency. At the moment, it is not available on crates.io since it is still under heavy development.
This library currently re-exports the `godot` crate, but adding the `godot` dependency as well is recommended, as this most likely will change in the future.

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
godot-rust-script = { git  = "https://github.com/TitanNano/godot-rust-script.git", branch = "master" }
```

## Bootstrap Script Runtime

The script runtime has to be registered with the engine, as Godot does not know how scripts written in rust should be executed or even that 
it's available as a scripting language.

For this, a manual implementation of the `godot::init::ExtensionLibrary` trait is required. Initializing and deinitalizing the runtime can then
be achieved via two macro calls. The `init!(...)` macro requires the name / path to a module in your library, which represents the root module
of all available scripts.

```rust
struct Lib;

#[gdextension]
unsafe impl ExtensionLibrary for Lib {
    fn on_level_init(level: InitLevel) {
        match level {
            InitLevel::Core => (),
            InitLevel::Servers => (),
            InitLevel::Scene => godot_rust_script::init!(scripts),
            InitLevel::Editor => (),
        }
    }

    fn on_level_deinit(level: InitLevel) {
        match level {
            InitLevel::Editor => (),
            InitLevel::Scene => godot_rust_script::deinit!(),
            InitLevel::Servers => (),
            InitLevel::Core => (),
        }
    }
}
```

## Define Scripts Root

Rust scripts require a root module. All rust modules under this module will be considered as potential scripts.

```rust
mod example_script;

godot_rust_script::define_script_root!();
```

## Write the first Script

Godots script system is file-based, which means each of your rust scripts has to go into its own module file. Rust script then uses the name
of your module file (e.g., `player_controller.rs`) to identify the script class / struct inside of it (e.g., `PlayerController`). 

Currently, all scripts are defined as global classes in the engine. This means each script must have a unique name.

Scripts are then composed of a `struct` definition and an `impl` block. Public functions inside the impl block will be made available to 
other scripting languages and the engine, so they must use Godot compatible types. The same applies to struct fields. 
Struct fields can additionally be exported via the `#[export]` attribute, so they show up in the editor inspector.

```rust
use godot_rust_script::{
	godot::prelude::{godot_print, Gd, GodotString, Node3D, Object},
	godot_script_impl, GodotScript,
};

#[derive(Debug, GodotScript)]
struct ExampleScript {
	#[export]
	pub flag: bool,
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

# FAQ

## Can I write / edit scripts in the godot editor?
No, it's currently neither supported nor planned. There are numerous good Rust editors and IDEs, so supporting the language inside 
the Godot code editor is not a goal of this project.

## Can I compile my scripts from inside the godot editor?
This is currently not supported.
