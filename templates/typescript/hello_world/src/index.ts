interface Input {
  name: string;
}

interface Output {
  message: string;
}

function main(): void {
  const args = process.argv.slice(2);
  if (args.length < 1) {
    console.error("Usage: node dist/index.js <json_input>");
    process.exit(1);
  }

  const input: Input = JSON.parse(args[0]);
  const output: Output = {
    message: `Hello, ${input.name}!`
  };

  console.log(JSON.stringify(output));
}

main();
