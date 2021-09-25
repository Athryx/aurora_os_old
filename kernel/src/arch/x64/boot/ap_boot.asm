extern AP_START_DATA
extern _ap_start

section .ap_text
bits 16
ap_start:
	cli
	lgdt [gdt32.pointer]
	mov eax, cr0	; set bit 1 in cr0 to enable protected 32 bit mode
	or eax, 1
	mov cr0, eax
	jmp 0x8:ap_32	; perform long jump with code selector to jump to 32 bit code

align 32
bits 32
ap_32:
; put pml4 address in cr3
	mov eax, [AP_START_DATA],
	mov cr3, eax

; enable pae bit
	mov eax, cr4
	or eax, 1 << 5
	mov cr4, eax

; set long mode bit in efer msr
	mov ecx, 0xc0000080
	rdmsr
	or eax, 1 << 8
	wrmsr

; set paging bit
	mov eax, cr0
	or eax, 1 << 31
	mov cr0, eax

; load temporary 64 bit gdt and long jump to long mode
	lgdt [gdt64.pointer]
	jmp gdt64.code:ap_long_mode_start

align 32
bits 64
long_mode_start:
; zero all segment registers
	mov ax, 0
	mov ss, ax
	mov ds, ax
	mov es, ax
	mov fs, ax
	mov gs, ax

; get processor id
	mov rax, 1
	cpuid
	shr rbx, 24
	and rbx, 0xff

; get stack pointer from AP_START_DATA
	mov rax, AP_START_DATA
	add rax, 8				; skip over the cr3 field adn the unused field in AP_START_DATA
	dec rbx					; decrement rbx so id 0 is first ap, otherwise bsp will be id 0 and 1 stack will be wasted
	mov rsp, [rax + 8 * rbx]

; call rust ap code
	mov rax, _ap_start
	call rax

align 32
gdt32:
	dq 0
	dq 0x00cf9a000000ffff	; 32 bit code
	dq 0x008f92000000ffff	; 32 bit data
	dq 0x00cf890000000068	; tss
.pointer
	dw .pointer - gdt32 - 1
	dq gdt32

align 32
gdt64:		; 64 bit global descriptor table
	dq 0
.code equ $ - gdt64
	dq (1 << 43) | (1 << 44) | (1 << 47) | (1 << 53)
.pointer:	; data structure to pass to lgdt
	dw .pointer - gdt64 - 1
	dq gdt64
