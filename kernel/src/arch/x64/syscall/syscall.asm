%include "asm_def.asm"

global syscall_entry

extern syscalls
;extern _ZN5sched8thread_cE	; FIXME: change this to extern "C" in c++ code

section .text
bits 64
syscall_entry:
	; kernel stack pointer should be 16 byte aligned
	; iretq clears gs_base msr, so we have to keep it in gs_base_k, and use swapgs to access it
	swapgs
	mov r10, rsp
	mov [gs:gs_data.call_save_rsp], rsp	; save caller rsp
	mov rsp, [gs:gs_data.call_rsp]		; load kernel rsp

	push r11		; save old flags
	push r10		; save old rsp

	;mov r11, [gs:gs_data.call_save_rsp]	; need to update call_save_rsp in thread data structure
	;mov r10, _ZN5sched8thread_cE		; need to do this to get around relocation issue
	;add r10, registers.call_save_rsp
	;mov [r10], r11
	swapgs
	sti

	push rcx		; save return rip

	sub rsp, 8		; for 16 byte alignment when calling c function
	push r15		; push args on stack, needed here because rax is used
	push r14
	push r13
	push r12
	push rax
	push rdi

	mov rax, rsi
	shl rax, 32		; cant use and because it messes things up
	shr rax, 32

	cmp rax, 15		; make sure it is a valid syscall
	jg .invalid_syscall

	mov rcx, rbx		; move argument 2 into place

	shr rsi, 32

	lea rdi, [rsp + 0x38]	; move syscall_vals_t structure pointer into place

	mov r10, syscalls
	mov rax, [r10 + rax * 8]
	call rax		; stack is already 16 byte aligned

	jmp .valid_syscall

.invalid_syscall:
	mov rax, -1 
	mov rdx, -1

.valid_syscall:
	add rsp, 16		; put stack pointer to right place
	pop r12
	pop r13
	pop r14
	pop r15
	add rsp, 8

	pop rcx			; restore return rip
	pop r10			; read old rsp
	pop r11			; restore flags

	cli
	swapgs
	mov rsp, [gs:gs_data.call_save_rsp]	; load save rsp
	swapgs
	o64 sysret
