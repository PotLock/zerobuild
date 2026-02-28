package main

import (
	"encoding/json"
	"fmt"
	"os"
	"strings"
)

type Input struct {
	Text string `json:"text"`
}

type Output struct {
	Count int `json:"count"`
}

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintf(os.Stderr, "Usage: %s <json_input>\n", os.Args[0])
		os.Exit(1)
	}

	var input Input
	if err := json.Unmarshal([]byte(os.Args[1]), &input); err != nil {
		fmt.Fprintf(os.Stderr, "Error parsing input: %v\n", err)
		os.Exit(1)
	}

	words := strings.Fields(input.Text)
	output := Output{Count: len(words)}
	
	result, _ := json.Marshal(output)
	fmt.Println(string(result))
}
