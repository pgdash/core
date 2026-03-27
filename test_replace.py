import re

with open('src/scanner/mod.rs', 'r') as f:
    content = f.read()

# Instead of blindly replacing, let's look at specific occurrences.
print("Before:")
for match in re.finditer(r'entry\(([^)]+)\.clone\(\)\)', content):
    print(f"Found: {match.group(0)}")
