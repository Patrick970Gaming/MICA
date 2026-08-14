import json
import os
import struct
from typing import TypedDict


# Settings
add_halt = True
output_binary = True
verbose = True
stack_size = 512
debug_code_output_len = 50 # When it comes to outputting the byte array of the finished code this is how many bytes get displayed
debug_code_output_start = 0 # Starting address of the above

#Constants
dir_path = os.path.dirname(os.path.realpath(__file__))

instruction_set_full = [
    "NOP",     "LDA",     "LDAI",     "LDB",     "LDBI",     "LDD",     "LDE",     "STA",     "STAI",     "STB",     "STBI",     "STC",     "STD",     "STE",     "PSH",     "PLL",
    "ADD",     "SUB",     "MUL",     "DIV",     "FADD",     "FSUB",     "FMUL",     "FDIV",     "JMP",     "JMPE",     "JMPN",     "JMPG",     "JMPGU",     "JMPL",     "JMPLU",     "CMP",
    "SHR",     "SHL",     "AND",     "OR",     "NOT",     "XOR",     "NEG",     "RET",     "HAL"
]

instruction_set_fullI = ["NOP", "LDA", "LDAI", "LDB", "LDBI", "LDD", "LDE", "STA", "STAI", "STB", "STBI", "STC", "STD", "STE", "PSH", "PLL",
    "ADD", "SUB", "MUL", "DIV", "JMP", "JMPE", "JMPN", "JMPG", "JMPGU", "JMPL", "JMPLU", "CMP", "SHR", "SHL", "AND", "OR", "NOT", "XOR",
    "NEG", "RET", "HAL"]

# Tool functions
def is_power_of_two(n: int) -> bool:
    return n > 0 and n.bit_count() == 1

def parse_decimal_literal(token: str) -> int:
    """Parses a '#'-prefixed decimal literal into the raw 32-bit
    pattern that should be stored/loaded. Integers pass through
    as-is; anything with a decimal point is packed as an IEEE-754
    float bit pattern so FADD/FSUB/FMUL/FDIV can operate on it."""
    text = token.replace("#", "")
    if "." in text:
        return struct.unpack(">I", struct.pack(">f", float(text)))[0]
    return int(text)

# Read in config file
# Open and parse the JSON file
with open(os.path.join(dir_path, "config.json"), "r") as file:
    read_config = json.load(file)

print(f"Active config: \n{json.dumps(read_config, indent=4)}")

# Reads in config
bitwidth = read_config["bit_width"]

if not is_power_of_two(bitwidth):
    raise ValueError("Configured bit width is not a power of two")

instruction_standard = read_config["instruction_set_standard"]

instruct_array: list[str]
if instruction_standard == "full":
    instruct_array = instruction_set_full
elif instruction_standard == "full_int":
    instruct_array = instruction_set_fullI
else:
    raise ValueError("Instruction set standard missing from config")

verbose = read_config["verbose"]


instruct_dict: dict[str, int]  = {}
for i in range(len(instruct_array)):
    instruct_dict[instruct_array[i]] = i

if verbose: print(f"intrsut_dict: {instruct_dict}")

Assembled_file = os.path.join(dir_path, "assembly.masm")
outputbin = os.path.join(dir_path, f"output{bitwidth}.bin")

try:
    with open(Assembled_file, "r") as file:
        raw_assembly_file = file.read()
except FileNotFoundError:
    raise ValueError("Could not find assembly.masm")

assembly_lines = raw_assembly_file.split("\n")


# remove empty lines from assembly lines
assembly_lines = [x for x in assembly_lines if x.strip()]

#print(f"Assemly lines {assembly_lines}")

#Process varaibles in the assembly and save to a dictonary
num_vars = 0
varibles = {}

for line_num in range(len(assembly_lines)):
    if assembly_lines[line_num][0] == "!":
        value = 0 # sets defualt to 0 so if there is no value for varible defined it defaults to 0
        p1 = assembly_lines[line_num].replace("!", "").split(" ")
        name = p1[0]
        if p1[1][0] == "#": # get value from decimal value
            value = parse_decimal_literal(p1[1])
        elif p1[1][0] == "$": # get value from hexadecimal value
            value = int(p1[1].replace("$", ""), 16)

        varibles[name] = {"value": value, "var_num": num_vars}
        num_vars += 1

# Process labels/functions and store into a dictonary
class LabelEntry(TypedDict):
    fun_num: int
    code: list[str]


num_lables = 0
labels: dict[str, LabelEntry] = {}

for line_num in range(len(assembly_lines)):
    if assembly_lines[line_num][0] == "@":
        name = assembly_lines[line_num].replace("@", "")
        code: list[str] = []
        for i in range(line_num + 1, len(assembly_lines)):
            if assembly_lines[i][0] not in ["@"]:
                code.append(assembly_lines[i].replace("\t", ""))
            elif assembly_lines[i][0] in ["@"]:
                break
        if add_halt and name == "main":
            code.append("    HAL")

        labels[name] = {"fun_num": num_lables, "code": code}
        num_lables += 1

if verbose:
    print(f"Varibles: {varibles}")
    print(f"Labels: {labels}")

label_lengths = {}

# calc length of labels
for function in labels:
    code = labels[function]['code']
    code_bytes: list[int] = []
    for line in code:
        if line[:4] == "    ":
            splited = line[4:].split(" ")
        else:
            splited = line.split(" ")
        if verbose: print(f"splitted (Calc len): {splited}") # output debugging information
        if len(splited) > 1:
            opcode = splited[0]
            if verbose: print(f"opcode (Calc len): {opcode}")
            opcode = instruct_dict[opcode]
            #for i in range(3): code_bytes.append(0)
            code_bytes.append(opcode)

            data = splited[1]
            if data[0] == "#": # get value from decimal value (parameter is decimal)
                data = parse_decimal_literal(data)
            elif data[0] == "$": # get value from hexadecimal value (parameter is hexadecimal)
                data = int(data.replace("$", ""), 16)
            elif data[0] == "!": # parameter is varaible
                #data = data[1:]
                data = 0
            elif data[0] == "@": # parameter is label
                data = 0
            else:
                raise ValueError(f"{data} is not valid in {labels[function]}")

            if verbose: print(f"data (Calc len): {data}")
            """
            code_bytes.append((4278190080 & value) >> 24)
            code_bytes.append((16711680 & value) >> 16)
            code_bytes.append((65280 & data) >> 8)
            """
            code_bytes.append(255 & data)

        elif len(splited) == 0 or splited[0] != "":
            opcode = splited[0]
            if verbose: print(f"opcode (Calc len): {opcode}")
            opcode = instruct_dict[opcode]
            #for i in range(3): code_bytes.append(0)
            code_bytes.append(opcode)
    label_lengths[function] = len(code_bytes)

if verbose: print(f"Label lenths: {label_lengths}")

total_function_len = 0
for fun in label_lengths:
    total_function_len += label_lengths[fun]

if verbose: print(f"Total length of functions: {total_function_len}")

# Precompute each label's starting word-offset as the sum of every
# label's length that comes before it, in declaration order.
label_offsets = {}
running_offset = 0
for name in labels:
    label_offsets[name] = running_offset
    running_offset += label_lengths[name]

if verbose: print(f"Label offsets: {label_offsets}")

output_code = []

# porcess labelsas
processed_labels = []
for function in labels:
    code = labels[function]['code']
    if verbose: print(f"code (processing labels): {code}")
    code_bytes = []
    for line in code:
        if line[:4] == "    ":
            splited = line[4:].split(" ")
        else:
            splited = line.split(" ")
        if verbose: print(f"splitted (processing labels): {splited}")
        if len(splited) > 1:
            opcode = splited[0]
            if verbose: print(f"opcode (processing labels): {opcode}")
            opcode = instruct_dict[opcode]
            for i in range(3): code_bytes.append(0)
            code_bytes.append(opcode)

            data = splited[1]
            if data[0] == "#": # get value from decimal value (parameter is decimal)
                data = parse_decimal_literal(data)
                value = data
                data = hash(str(value))
                if data not in varibles:
                    previous_var = list(varibles.keys())[-1]
                    varibles[data] = {"value": value, "var_num": varibles[previous_var]["var_num"] + 1}
                    data = varibles[data]["var_num"] + total_function_len
                else:
                    data = varibles[data]["var_num"] + total_function_len
                code_bytes.append((4278190080 & data) >> 24)
                code_bytes.append((16711680 & data) >> 16)
                code_bytes.append((65280 & data) >> 8)
                code_bytes.append(255 & data)
            elif data[0] == "$": # get value from hexadecimal value (parameter is hexadecimal)
                data = int(data.replace("$", ""), 16)
                value = data
                data = hash(str(value))
                if data not in varibles:
                    previous_var = list(varibles.keys())[-1]
                    varibles[data] = {"value": value, "var_num": varibles[previous_var]["var_num"] + 1}
                    data = varibles[data]["var_num"] + total_function_len
                else:
                    data = varibles[data]["var_num"] + total_function_len
                code_bytes.append((4278190080 & data) >> 24)
                code_bytes.append((16711680 & data) >> 16)
                code_bytes.append((65280 & data) >> 8)
                code_bytes.append(255 & data)
            elif data[0] == "!": # parameter is varaible
                data = data[1:]
                data = varibles[data]["var_num"] + total_function_len
                code_bytes.append((4278190080 & data) >> 24)
                code_bytes.append((16711680 & data) >> 16)
                code_bytes.append((65280 & data) >> 8)
                code_bytes.append(255 & data)
            elif data[0] == "@": # parameter is label
                data = data[1:]
                data = label_offsets[data]
                code_bytes.append((4278190080 & data) >> 24)
                code_bytes.append((16711680 & data) >> 16)
                code_bytes.append((65280 & data) >> 8)
                code_bytes.append(255 & data)
            else:
                print(f"{data} is not valid in {labels[function]}")


            if verbose: print(f"data (processing labels): {data}")
        elif len(splited) == 0 or splited[0] != "":
            opcode = splited[0]
            if verbose: print(f"opcode (processing labels): {opcode}")
            opcode = instruct_dict[opcode]
            for i in range(3): code_bytes.append(0)
            code_bytes.append(opcode)

    processed_labels.append({function: {"code": code_bytes}})
if verbose: print(f"Process_labels: {processed_labels}")

# Add functions to the image:

for fun_counter, label in enumerate(processed_labels):
    label_name = next(iter(label.keys()))
    if fun_counter == 0:
        start_address = labels[label_name]["fun_num"]
    if fun_counter > 0:
        start_address = label_lengths[list(label_lengths.keys())[fun_counter - 1]]

    code = label[label_name]["code"]
    if verbose: print(f"Code of {label_name}: {code}")
    for code_index in range(len(code)):
        output_code.append(code[code_index])

# Add varaible refernces to the image:
for var in varibles:
    address = (varibles[var]['var_num'] * 2) + total_function_len
    value = varibles[var]['value']
    output_code.append((4278190080 & value) >> 24)
    output_code.append((16711680 & value) >> 16)
    output_code.append((65280 & value) >> 8)
    output_code.append(255 & value)

if verbose:
    print(f"First {debug_code_output_len} bytes of code:")
    print(output_code[debug_code_output_start:(debug_code_output_start + debug_code_output_len)])

print(varibles)

if output_binary:
    with open(outputbin, "wb") as f:
        _ = f.write(bytearray(output_code))
        f.close()
