use core::ops::{Deref, DerefMut};

use spin::{mutex::Mutex, MutexGuard};

use crate::arch::interrupts::{are_interrupts_enabled, disable_interrupts, enable_interrupts};

/* A lock for things that may be accessed from an interrupt handler.
 * In this scenario, an interrupt may arrive while the lock is held, and the handler may attempt to take
 * the lock, deadlocking the handler.
 * To prevent this, this lock disables interrupts while it is held.
 * Don't hold this for too long.
 */

pub struct NoInterruptMutexGuard<'a, T> {
    interrupts_enabled_on_enter: bool,
    guard: MutexGuard<'a, T>,
}

impl<'a, T> Drop for NoInterruptMutexGuard<'a, T> {
    fn drop(&mut self) {
        if self.interrupts_enabled_on_enter {
            enable_interrupts();
        }
    }
}

impl<'a, T> Deref for NoInterruptMutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.guard.deref()
    }
}

impl<'a, T> DerefMut for NoInterruptMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.deref_mut()
    }
}

pub struct InterruptSafeSpinlock<T>(Mutex<T>);

impl<T> InterruptSafeSpinlock<T> {
    pub const fn new(value: T) -> Self {
        Self(Mutex::new(value))
    }

    pub fn lock<'a>(&'a self) -> NoInterruptMutexGuard<'a, T> {
        let was_enabled = are_interrupts_enabled();

        if was_enabled {
            disable_interrupts();
        }

        NoInterruptMutexGuard {
            interrupts_enabled_on_enter: was_enabled,
            guard: self.0.lock(),
        }
    }
}

unsafe impl<T> Send for InterruptSafeSpinlock<T> {}
unsafe impl<T> Sync for InterruptSafeSpinlock<T> {}
