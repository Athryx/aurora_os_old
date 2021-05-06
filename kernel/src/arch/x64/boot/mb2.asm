section .multiboot_header
hstart:
	dd 0xe85250d6
	dd 0			; Arhitecture
	dd hend - hstart	; Header Length
	dd 0x100000000 - (0xe85250d6 + 0 + (hend - hstart))

	dw 0
	dw 0
	dd 8			; End Tag
hend:
