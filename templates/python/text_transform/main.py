#!/usr/bin/env python3
import json
import sys

def main():
    if len(sys.argv) < 2:
        print(f"Usage: {sys.argv[0]} <json_input>", file=sys.stderr)
        sys.exit(1)
    
    input_data = json.loads(sys.argv[1])
    text = input_data.get("text", "")
    operation = input_data.get("operation", "upper")
    
    if operation == "upper":
        result = text.upper()
    elif operation == "lower":
        result = text.lower()
    elif operation == "reverse":
        result = text[::-1]
    else:
        result = text
    
    output = {"result": result}
    print(json.dumps(output))

if __name__ == "__main__":
    main()
