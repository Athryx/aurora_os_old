ENTRY(start)

KERNEL_VMA = 0xffffff8000000000;

SECTIONS
{
	. = 4M;

	KERNEL_LMA = .;
	. += KERNEL_VMA;

	.text ALIGN(4K) : AT(ADDR(.text) - KERNEL_VMA)
	{
		TEXT_START = .;
		*(.text)
		TEXT_END = .;
	}

	.rodata ALIGN(4K) : AT(ADDR(.rodata) - KERNEL_VMA)
	{
		RODATA_START = .;
		*(.rodata)
		RODATA_END = .;
	}

	.data ALIGN(4K) : AT(ADDR(.data) - KERNEL_VMA)
	{
		DATA_START = .;
		*(.data)
		DATA_END = .;
	}

	.bss ALIGN(4K) : AT(ADDR(.bss) - KERNEL_VMA)
	{
		BSS_START = .;
		*(.bss)
		BSS_END = .;
	}
	. = ALIGN(4K);
	KERNEL_END = .;

	/DISCARD/ :
	{
		*(.comment)
		*(.eh_frame)
	}
}
