import { readFile, writeFile } from 'node:fs/promises';
import readline from 'node:readline';

const types = ['fix', 'feature', 'refactor'];

async function selectWithArrows() {
  const input = process.stdin;
  const output = process.stderr;
  let selected = 0;

  function render() {
    readline.cursorTo(output, 0);
    readline.clearScreenDown(output);
    output.write('Is this a fix, feature, or refactor? (up/down, Enter)\n');
    for (const [index, type] of types.entries()) {
      output.write(`${index === selected ? '> ' : '  '}${type}\n`);
    }
    readline.moveCursor(output, 0, -(types.length + 1));
  }

  readline.emitKeypressEvents(input);
  input.setRawMode(true);
  input.resume();
  render();

  return new Promise((resolve, reject) => {
    function finish(error) {
      input.removeListener('keypress', onKey);
      input.removeListener('end', onEnd);
      input.setRawMode(false);
      input.pause();
      readline.cursorTo(output, 0);
      readline.clearScreenDown(output);
      if (error) reject(error);
      else {
        output.write(`Type: ${types[selected]}\n`);
        resolve(types[selected]);
      }
    }

    function onKey(_character, key) {
      if (key?.ctrl && key.name === 'c') {
        finish(new Error('Commit cancelled.'));
      } else if (key?.name === 'up' || key?.name === 'down') {
        selected = (selected + (key.name === 'up' ? -1 : 1) + types.length) % types.length;
        render();
      } else if (key?.name === 'return' || key?.name === 'enter') {
        finish();
      }
    }

    function onEnd() {
      finish(new Error('Interactive input ended; commit cancelled.'));
    }

    input.on('keypress', onKey);
    input.once('end', onEnd);
  });
}

async function main() {
  const [messageFile, source] = process.argv.slice(2);
  if (!messageFile) throw new Error('Missing Git commit message file.');

  const existing = await readFile(messageFile, 'utf8');
  const descriptionDefault = source && source !== 'template'
    ? existing.replace(/\s+/g, ' ').trim()
    : '';
  const arrowMenu = process.stdin.isTTY && process.stderr.isTTY;
  let type = arrowMenu ? await selectWithArrows() : undefined;

  const rl = readline.createInterface({
    input: process.stdin,
    output: process.stderr,
    terminal: Boolean(arrowMenu),
  });
  const lines = rl[Symbol.asyncIterator]();

  async function ask(prompt) {
    rl.setPrompt(prompt);
    rl.prompt();
    const result = await lines.next();
    if (result.done) throw new Error('Interactive input ended; commit cancelled.');
    return result.value.trim();
  }

  try {
    while (!type) {
      const answer = await ask('Is this a fix, feature, or refactor? (1/2/3): ');
      if (answer === '1' || answer === '2' || answer === '3') {
        type = types[Number(answer) - 1];
      } else {
        process.stderr.write('Choose 1, 2, or 3.\n');
      }
    }

    let scope = '';
    while (!scope) {
      scope = await ask('What part is affected? ');
      if (!scope) process.stderr.write('The affected part is required.\n');
    }

    let description = '';
    while (!description) {
      description = await ask(
        descriptionDefault ? `Describe the change [${descriptionDefault}]: ` : 'Describe the change: ',
      );
      description ||= descriptionDefault;
      if (!description) process.stderr.write('A description is required.\n');
    }

    await writeFile(messageFile, `${type}(${scope}): ${description}\n`, 'utf8');
  } finally {
    rl.close();
  }
}

main().catch((error) => {
  process.stderr.write(`Commit message hook: ${error.message}\n`);
  process.exitCode = 1;
});
