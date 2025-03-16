input_program = [[1, 1], [2, 1], [6]]

program = list()

"""
Start file with a function called function one. this corridsponds to the starting memoery address.
When a jump occurs to a certain meomory address it will call the function that corrosponds to that memory address.
This will not be done recusrively 

There will be a main loop and the jumps will be conditonally done in this loop. Not inside the emulation functions 

Option 2;
Do a run through the whole input program and check for the first jump. Then make a function from the jump desitination to the jump. 
This gets done to every jump in the input_program. any left over code at the start of the program is put into the main function. 
"""

for instruction in input_program:
    if instruction[0] == 1: # LDA
        program.append(f"reg_a = {instruction[1]}")
    elif instruction[0] == 2: # LDB
       program.append(f"reg_b = {instruction[1]}")
    elif instruction[0] == 3: # LDC
        program.append(f"reg_c = {instruction[1]}")
    elif instruction[0] == 4: # STA
        variable_name = f"var_{instruction[1]}"
        program.append(f"{variable_name} = reg_a")
    elif instruction[0] == 5: # STB
        variable_name = f"var_{instruction[1]}"
        program.append(f"{variable_name} = reg_b")
    elif instruction[0] == 6: # ADD
       program.append(f"reg_a = reg_a + reg_b")
    elif instruction[0] == 7: # SUB
       program.append(f"reg_a = reg_a - reg_b")
    elif instruction[0] == 8: # MUL
       program.append(f"reg_a = reg_a * reg_b")
    elif instruction[0] == 9: # DIV
       program.append(f"reg_a = reg_a // reg_b")

#print(program)
program = "\n".join(program)
print(program)
