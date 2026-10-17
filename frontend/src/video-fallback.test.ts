import { afterEach, beforeEach, describe, expect, test } from "vitest";

import { setup, teardown } from "./video-fallback";

const URL = "https://github.com/user-attachments/assets/2f1c9e0a";

describe("video fallback", () => {
  beforeEach(() => {
    document.body.innerHTML = `
      <article class="markdown-body">
        <p><video src="${URL}" controls muted><a href="${URL}">${URL}</a></video></p>
      </article>`;
    setup();
  });

  afterEach(() => {
    teardown();
    document.body.innerHTML = "";
  });

  test("a video that cannot play leaves the link it holds", () => {
    document.querySelector("video")!.dispatchEvent(new Event("error"));

    expect(document.querySelector("video")).toBeNull();
    expect(document.querySelector("p > a")?.getAttribute("href")).toBe(URL);
  });

  test("a video that plays keeps its player", () => {
    document.querySelector("video")!.dispatchEvent(new Event("loadeddata"));

    expect(document.querySelector("video")).not.toBeNull();
  });

  test("a video with nothing to fall back to stays as it is", () => {
    document.body.innerHTML = `<article class="markdown-body"><video src="${URL}"></video></article>`;
    document.querySelector("video")!.dispatchEvent(new Event("error"));

    expect(document.querySelector("video")).not.toBeNull();
  });
});
