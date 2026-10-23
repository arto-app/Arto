import fs from "fs";
import os from "os";
import path from "path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";

import { writeIfChanged } from "./write-if-changed.ts";

describe("writeIfChanged", () => {
  let dir: string;

  beforeEach(() => {
    dir = fs.mkdtempSync(path.join(os.tmpdir(), "write-if-changed-"));
  });

  afterEach(() => {
    fs.rmSync(dir, { recursive: true, force: true });
  });

  it("creates the file and the directories above it", () => {
    const file = path.join(dir, "nested/out.txt");

    expect(writeIfChanged(file, "content")).toBe(true);
    expect(fs.readFileSync(file, "utf-8")).toBe("content");
  });

  it("leaves a file with the same content untouched", () => {
    const file = path.join(dir, "out.txt");
    fs.writeFileSync(file, "content");
    const past = new Date(2000, 0, 1);
    fs.utimesSync(file, past, past);

    expect(writeIfChanged(file, "content")).toBe(false);
    expect(fs.statSync(file).mtime).toEqual(past);
  });

  it("rewrites a file whose content differs", () => {
    const file = path.join(dir, "out.txt");
    fs.writeFileSync(file, "old");

    expect(writeIfChanged(file, "new")).toBe(true);
    expect(fs.readFileSync(file, "utf-8")).toBe("new");
  });
});
