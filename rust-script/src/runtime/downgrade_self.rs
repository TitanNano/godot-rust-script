/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

use godot::{
    builtin::meta::ToGodot,
    obj::{Gd, GodotClass, Inherits, WithBaseField},
};

pub trait DowngradeSelf: GodotClass {
    fn downgrade_gd<F: FnOnce(Gd<Self>) -> R, R>(&mut self, closure: F) -> R;
}

impl<T> DowngradeSelf for T
where
    T: WithBaseField + GodotClass,
    T: Inherits<<T as GodotClass>::Base>,
{
    fn downgrade_gd<F: FnOnce(Gd<Self>) -> R, R>(&mut self, closure: F) -> R {
        let mut_base = self.base_mut();
        let self_gd = mut_base.to_godot().cast();

        closure(self_gd)
    }
}
