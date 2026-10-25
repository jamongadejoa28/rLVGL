/* Minimal Cortex-M linker script for the ferrovgl size probe (rust-lld).
 * The memory sizes only need to be large enough to link; they do not
 * affect the measured section sizes. */

MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 2048K
  RAM   : ORIGIN = 0x20000000, LENGTH = 256K
}

ENTRY(Reset);
EXTERN(RESET_VECTOR);

_stack_start = ORIGIN(RAM) + LENGTH(RAM);

SECTIONS
{
  /* Initial stack pointer and reset vector: the only roots for --gc-sections. */
  .vector_table ORIGIN(FLASH) :
  {
    LONG(_stack_start);
    KEEP(*(.vector_table.reset_vector));
  } > FLASH

  .text : ALIGN(4)
  {
    *(.text .text.*);
    . = ALIGN(4);
  } > FLASH

  .rodata : ALIGN(4)
  {
    *(.rodata .rodata.*);
    . = ALIGN(4);
  } > FLASH

  .data : ALIGN(4)
  {
    __sdata = .;
    *(.data .data.*);
    . = ALIGN(4);
    __edata = .;
  } > RAM AT > FLASH

  __sidata = LOADADDR(.data);

  .bss (NOLOAD) : ALIGN(4)
  {
    __sbss = .;
    *(.bss .bss.*);
    *(COMMON);
    . = ALIGN(4);
    __ebss = .;
  } > RAM

  /* Unwinding tables are unused with panic = "abort". */
  /DISCARD/ :
  {
    *(.ARM.exidx);
    *(.ARM.exidx.*);
    *(.ARM.extab.*);
  }
}
