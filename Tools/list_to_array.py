# This script transforms the list from the opcode spreadsheet into a valid array format
import os

dir_path = os.path.dirname(os.path.realpath(__file__))

with open(os.path.join(dir_path, "opcodes.txt"), "r") as f:
    opcodes = [line.strip() for line in f if line.strip()]

print("[")
for i, opcode in enumerate(opcodes):
    end = ", " if i != len(opcodes) - 1 else ""
    if (i + 1) % 16 == 0:
        print(f'    "{opcode}"{end}')
    else:
        print(f'    "{opcode}"{end}', end="")
print("\n]")
