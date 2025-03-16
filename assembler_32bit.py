import os

# Settings
add_halt = True
output_binary = True
verbose = True
stack_size = 512
debug_code_output_len = 50 # When it comes to outputting the byte array of the finished code this is how many bytes get displayed
debug_code_output_start = 0 # Starting address of the above

# Instruction dictonary 
#instruction array, used to create insutrction dictonary
instruct_array = ["NOP", "LDA", "LDB", "LDD", "STA", "STB", "STC", "STD", "PSH", "PLL", "ADD", "SUB", "MUL", "DIV", "FADD", "FSUB", "FMUL", "FDIV", "JMP", "JMPE", "JMPN", "JMPG", "JMPGU", "JMPL", "JMPLU", "CMP", "SHR", "SHL", "AND", "OR", "NOT", "XOR", "NEG", "RET", "HAL"] 

instruct_dict = {}
for i in range(len(instruct_array)):
    instruct_dict[instruct_array[i]] = i

if verbose: print(instruct_dict)

dir_path = os.path.dirname(os.path.realpath(__file__))
Assembled_file = os.path.join(dir_path, "assembly.txt")
outputbin = os.path.join(dir_path, "output32.bin")

raw_assembly_file = open(Assembled_file, "r")
raw_assembly_text = raw_assembly_file.read()

assembly_lines = raw_assembly_text.split("\n")


# remove empty lines from assembly lines
assembly_lines = [x for x in assembly_lines if x.strip()]

#print(f"Assemly lines {assembly_lines}")

num_vars = 0
varibles = {}

for line_num in range(len(assembly_lines)):
    if assembly_lines[line_num][0] == "!":
        p1 = assembly_lines[line_num].replace("!", "").split(" ")
        name = p1[0]
        if p1[1][0] == "#": # get value from decimal value
            value = int(p1[1].replace("#", ""))
        elif p1[1][0] == "$": # get value from hexadecimal value
            value = int(p1[1].replace("$", ""), 16)
        varibles[name] = {"value": value, "var_num": num_vars}
        num_vars += 1

num_lables = 0
labels = {}

for line_num in range(len(assembly_lines)):
    if assembly_lines[line_num][0] == "@":
        name = assembly_lines[line_num].replace("@", "")
        code = []
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
    code_bytes = []
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
            code_bytes.append(0)
            code_bytes.append(opcode)

            data = splited[1]
            if data[0] == "#": # get value from decimal value (parameter is decimal)
                data = int(data.replace("#", ""))
            elif p1[0] == "$": # get value from hexadecimal value (parameter is hexadecimal)
                data = int(data.replace("$", ""), 16)
            elif data[0] == "!": # parameter is varaible
                #data = data[1:]
                data = 0
            elif data[0] == "@": # parameter is label
                data = 0
            else:
                print(f"{data} is not valid in {labels[function]}")

            if verbose: print(f"data (Calc len): {data}")
            code_bytes.append((65280 & data) >> 8)
            code_bytes.append(255 & data)
        elif len(splited) == 0 or splited[0] != "":
            opcode = splited[0]
            if verbose: print(f"opcode (Calc len): {opcode}")
            opcode = instruct_dict[opcode]
            code_bytes.append(0)
            code_bytes.append(opcode)
    label_lengths[function] = len(code_bytes)

if verbose: print(f"Label lenths: {label_lengths}")

total_function_len = 0
for fun in label_lengths.keys():
    total_function_len += label_lengths[fun]

if verbose: print(f"Total length of functions: {total_function_len}")

output_code = list()

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
            code_bytes.append(0)
            code_bytes.append(opcode)

            data = splited[1]
            if data[0] == "#": # get value from decimal value (parameter is decimal)
                data = int(data.replace("#", ""))
                code_bytes.append((65280 & data) >> 8)
                code_bytes.append(255 & data)
            elif p1[0] == "$": # get value from hexadecimal value (parameter is hexadecimal)
                data = int(data.replace("$", ""), 16)
                code_bytes.append((65280 & data) >> 8)
                code_bytes.append(255 & data)
            elif data[0] == "!": # parameter is varaible
                data = data[1:]
                data = varibles[data]["var_num"] + total_function_len
                code_bytes.append((65280 & data) >> 8)
                code_bytes.append(255 & data)
            elif data[0] == "@": # parameter is label
                data = data[1:]
                label_num = labels[data]["fun_num"]
                if label_num == 0:
                    data = label_num
                else:
                    prev_fun = list(labels.keys())[label_num - 1]
                    data = (label_lengths[prev_fun] // 2) + 1
                code_bytes.append((65280 & data) >> 8)
                code_bytes.append(255 & data)
            else:
                print(f"{data} is not valid in {labels[function]}")


            if verbose: print(f"data (processing labels): {data}")
        elif len(splited) == 0 or splited[0] != "":
            opcode = splited[0]
            if verbose: print(f"opcode (processing labels): {opcode}")
            opcode = instruct_dict[opcode]
            code_bytes.append(0)
            code_bytes.append(opcode)

    processed_labels.append({function: {"code": code_bytes}})
if verbose: print(f"Process_labels: {processed_labels}")

# Add functions to the image:
fun_counter = 0
for label in processed_labels:
    if fun_counter == 0:
        label_name = list(label.keys())[0]
        start_address = labels[label_name]["fun_num"]
    if fun_counter > 0:
        start_address = label_lengths[list(label_lengths.keys())[fun_counter - 1]]
        
    code = label[list(label.keys())[0]]["code"]
    if verbose: print(f"Code of {list(label.keys())[0]}: {code}")
    for code_index in range(len(code)):
        output_code.append(code[code_index])

    fun_counter += 1

# Add varaible refernces to the image:
for var in varibles.keys():
    address = (varibles[var]['var_num'] * 2) + total_function_len
    value = varibles[var]['value']
    output_code.append((65280 & value) >> 8)
    output_code.append(255 & value)

if verbose:
    print(f"First {debug_code_output_len} bytes of code:")
    print(output_code[debug_code_output_start:(debug_code_output_start + debug_code_output_len)])

if output_binary:
    f = open(outputbin, "wb")
    f.write(bytearray(output_code))
    f.close()