/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/.
 */

pub trait Apply: Sized {
    fn apply<F: FnOnce(&mut Self)>(mut self, cb: F) -> Self {
        cb(&mut self);
        self
    }
}

impl<T: Sized> Apply for T {}
