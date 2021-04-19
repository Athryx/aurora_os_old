set disassembly-flavor intel
add-symbol-file target/x86_64-os/debug/rust_os
break _start
target remote localhost:1234
layout asm
layout next
