# MICA
This is MICA its is a RISC ISA created for educational purposes with the utlimate goal of one day booting linux 

## To Do
* Figure out how to get C to compile to this thing
* Create Interpreted Emulator
* Create recompiled Emulator
* Create FPGA "Emulator"

## Architecture information
### Instructions
| Heading           | Number | Hex | OPCODE | Description                                                                           |
|-------------------|--------|-----|--------|---------------------------------------------------------------------------------------|
|                   | 0      | 00  | NOP    | (no operation)                                                                        |
| Memory            | 1      | 01  | LDA    | (read data from RAM location into register A) parameter: 16-bit RAM   address         |
|                   | 2      | 02  | LDB    | (read data from RAM location into register B) parameter: 16-bit RAM   address         |
|                   | 3      | 03  | LDD    | (read data from RAM location into Flag register) parameter: 16-bit RAM   address      |
|                   | 4      | 04  | LDE    | (read data from RAM location into Segment register) parameter: 16-bit RAM   address   |
|                   | 5      | 05  | STA    | (write data from register A into RAM location) parameter: 16-bit RAM   address        |
|                   | 6      | 06  | STB    | (write data from register B into RAM location) parameter: 16-bit RAM   address        |
|                   | 7      | 07  | STC    | (write data from Accumlator into RAM location) parameter: 16-bit RAM   address        |
|                   | 8      | 08  | STD    | (write data from Flag register into RAM location) parameter: 16-bit RAM   address     |
|                   | 9      | 09  | STE    | (write data from Segment register into RAM location) parameter: 16-bit   RAM address  |
|                   | 10     | 0A  | PSH    | Push Value to stack                                                                   |
|                   | 11     | 0B  | PLL    | Pull Value from stack                                                                 |
| Math              | 12     | 0C  | ADD    | (add register A and B)                                                                |
|                   | 13     | 0D  | SUB    | (subtract register A and B)                                                           |
|                   | 14     | 0E  | MUL    | (Multiple register A and B)                                                           |
|                   | 15     | 0F  | DIV    | DIV (Divide register A and B)                                                         |
|                   | 16     | 10  | FADD   | FADD (add register A and B)                                                           |
|                   | 17     | 11  | FSUB   | (subtract register A and B)                                                           |
|                   | 18     | 12  | FMUL   | (Multiple register A and B)                                                           |
|                   | 19     | 13  | FDIV   | (Divide register A and B)                                                             |
| Conditionals      | 20     | 14  | JMP    | (jump to 16-bit address) parameter: 16-bit RAM address                                |
|                   | 21     | 15  | JMPE   | (jump to 16-bit address if equal flag set) parameter: 16-bit RAM address              |
|                   | 22     | 16  | JMPN   | (jump to 16-bit address if not equal) parameter: 16-bit RAM address                   |
|                   | 23     | 17  | JMPG   | (jump to 16-bit address if greater than) parameter: 16-bit RAM address                |
|                   | 24     | 18  | JMPGU  | (jump to 16-bit address if greater than unsigned) parameter: 16-bit RAM   address     |
|                   | 25     | 19  | JMPL   | (jump to 16-bit address if less than) parameter: 16-bit RAM address                   |
|                   | 26     | 1A  | JMPLU  | (jump to 16-bit address if less than unsigned) parameter: 16-bit RAM   address        |
|                   | 27     | 1B  | CMP    | (compare two memory address and set appropriate flags) parameter 16-bit   RAM address |
| Bitwise logic     | 28     | 1C  | SHR    | (Shift right register A)                                                              |
|                   | 29     | 1D  | SHL    | (Shift left register A)                                                               |
|                   | 30     | 1E  | AND    | (logical AND register A with register B)                                              |
|                   | 31     | 1F  | OR     | (logical OR register A with register B)                                               |
|                   | 32     | 20  | NOT    | (logical NOT register A)                                                              |
|                   | 33     | 21  | XOR    | (logical XOR register A with register B)                                              |
|                   | 34     | 22  | NEG    | (negate register A)                                                                   |
| Processor control | 35     | 23  | RET    | Return to jumped from function                                                        |
|                   | 36     | 24  | HAL    | Halt cpu                                                                              |