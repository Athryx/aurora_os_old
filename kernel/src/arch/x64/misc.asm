global asm_gs_addr

asm_gs_addr:
	swapgs
	mov rax, [gs:0]
	swapgs
	ret
