/* The qemu stack harness's memory map -- M52/T52.6
 * (docs/plan-embedded-footprint.md §2.6, src/bin/fwq.rs).
 *
 * `qemu-system-arm -M mps2-an386`: a Cortex-M4F with 4 MB of SSRAM at
 * 0x00000000, where the vector table must sit, and more at 0x20000000.
 * The code region stands in for flash. `.data` is linked straight into RAM
 * with no load address in flash, because qemu's ELF loader writes each
 * segment where it is linked and RAM starts zeroed: a board would need the
 * copy-and-zero a real reset handler does, and this harness is not a
 * board.
 *
 * The stack is the top STACK_SIZE bytes of RAM; fwq.rs paints and scans
 * [_stack_bottom, sp). Only fwq links this script (build.rs); the other
 * firmwares are measured for their sections and never run.
 */
MEMORY
{
  FLASH : ORIGIN = 0x00000000, LENGTH = 4M
  RAM   : ORIGIN = 0x20000000, LENGTH = 256K
}

STACK_SIZE = 64K;
_stack_top = ORIGIN(RAM) + LENGTH(RAM);
_stack_bottom = _stack_top - STACK_SIZE;

ENTRY(reset);

SECTIONS
{
  .vector_table ORIGIN(FLASH) :
  {
    LONG(_stack_top);
    KEEP(*(.vector_table.reset));
    KEEP(*(.vector_table.exceptions));
  } > FLASH

  .text : { *(.text .text.*); } > FLASH
  .rodata : { *(.rodata .rodata.*); } > FLASH
  .ARM.exidx : { *(.ARM.exidx .ARM.exidx.*); } > FLASH

  .data : { *(.data .data.*); } > RAM
  .bss (NOLOAD) : { *(.bss .bss.*); *(COMMON); } > RAM

  ASSERT(ADDR(.bss) + SIZEOF(.bss) <= _stack_bottom, "fwq: .data + .bss overlap the stack")

  /DISCARD/ : { *(.ARM.attributes.*) }
}
