import { afterEach, expect, test } from "vitest";

import { clear, find, setup } from "./find-in-page";

afterEach(() => {
  clear();
  document.body.innerHTML = "";
});

test("the link a playing video holds is not a match", () => {
  const url = "https://github.com/user-attachments/assets/2f1c9e0a";
  document.body.innerHTML = `
    <article class="markdown-body">
      <p>Uploaded to github.</p>
      <p><video src="${url}" controls><a href="${url}">${url}</a></video></p>
    </article>`;
  let count: number | undefined;
  setup((found) => {
    count = found.count;
  });

  find("github");

  expect(count).toBe(1);
});
