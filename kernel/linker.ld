ENTRY(start)

__KERNEL_VMA = 0xffffff8000000000;
__AP_CODE_START = 0x8000;

SECTIONS
{
	. = 1M;
	__KERNEL_START = __KERNEL_VMA + .;
	__KERNEL_LMA = .;

	/*.multiboot_header ALIGN(4K) :
	{
		*(.multiboot_header)
	}*/

	.boot_text ALIGN(4K) :
	{
		*(.boot_text)
	}

	.boot_rodata ALIGN(4K) :
	{
		*(.boot_rodata)
	}

	.boot_bss ALIGN(4K) :
	{
		*(.boot_bss)
	}

	. = ALIGN(4K);
	__END_BOOT = .;
	. = __AP_CODE_START;

	.ap_text ALIGN(4K) : AT(__END_BOOT)
	{
		*(.ap_text)
		__AP_CODE_END = .;
	}

	. = __END_BOOT + 4K + __KERNEL_VMA;

	.text ALIGN(4K) : AT(ADDR(.text) - __KERNEL_VMA)
	{
		__TEXT_START = .;
		*(.text*)
		__TEXT_END = .;
	}

	.rodata ALIGN(4K) : AT(ADDR(.rodata) - __KERNEL_VMA)
	{
		__RODATA_START = .;
		*(.rodata*)
		__RODATA_END = .;
	}

	.resources ALIGN(4K) : AT(ADDR(.resources) - __KERNEL_VMA)
	{
		*(.resources)
	}

	.data ALIGN(4K) : AT(ADDR(.data) - __KERNEL_VMA)
	{
		__DATA_START = .;
		*(.data*)
		__DATA_END = .;
	}

	.bss ALIGN(4K) : AT(ADDR(.bss) - __KERNEL_VMA)
	{
		__BSS_START = .;
		*(.bss*)
		__BSS_END = .;
	}
	. = ALIGN(4K);
	__KERNEL_END = .;

	/DISCARD/ :
	{
		*(.comment)
		*(.eh_frame)
		*(.eh_frame_hdr)
		/*(.debug*)*/
	}
}
