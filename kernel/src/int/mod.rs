pub mod idt;
pub mod pic;
pub mod apic;
pub mod manager;

crate::make_id_type!(Irq, u8);
