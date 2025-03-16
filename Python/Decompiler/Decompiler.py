import os 
import json

def decompile(opcode_path, program_path):
    with open(opcode_path) as opcodes_file:
        opcodes = json.load(opcodes_file)

    with open(program_path) as program_file:
        program_in = [line.rstrip() for line in program_file]

    #print(program_in)
    op_param = [1, 2, 4, 5, 11, 12, 13, 14, 15, 16]

    program = list()
    program_human = list()

    program_in_iter = iter(program_in)


    for line in program_in_iter:
        a = int(line, 2)
        
        if a in op_param:
            program.append([a, int(next(program_in_iter, None), 2)])
        else:
            program.append([a])

    program_in_iter = iter(program_in)
    if Human_read:
        print("Making Human")
        for line in program_in_iter:
            a = int(line, 2)
            if a in op_param:
                program_human.append([opcodes[str(a)], int(next(program_in_iter, None), 2)])
            else:
                program_human.append(opcodes[str(a)])

    if Debug:
        print(program_human)
        print(program)

if __name__ == "__main__":
    # Settings:
    Human_read = True # Produces parallel decompiled array that is human readable
    Debug = True # Enabel debug mode

    dir_path = os.path.dirname(os.path.realpath(__file__))
    opcode_path = os.path.join(dir_path, "opcodes.json")
    program_path = os.path.join(dir_path, "program.txt")

    decompile(opcode_path, program_path)