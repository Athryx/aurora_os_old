use crate::arch::x64::outb;
use pic::{PICM_OFFSET, PICM_COMMAND, PICS_COMMAND, PIC_EOI};
use crate::config;
use crate::kdata::cpud;

pub mod idt;
pub mod pic;
pub mod apic;
pub mod manager;

crate::make_id_type!(Irq, u8);

// tell pics interrupt is over, used by assembly code
#[no_mangle]
pub extern "C" fn eoi(irq: u8)
{
	if config::use_apic() {
		cpud().lapic().eoi();
	} else {
		if irq - PICM_OFFSET > 7 {
			outb(PICS_COMMAND, PIC_EOI);
		}
	
		outb(PICM_COMMAND, PIC_EOI);
	}
}
