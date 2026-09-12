import { describe, test, expect, beforeEach } from "vitest";

import { navigationDirection, onNavigate, setup } from "./mouse-navigation";

describe("navigationDirection", () => {
  test("reads the two side buttons as the two ways through the history", () => {
    expect(navigationDirection(3)).toBe("back");
    expect(navigationDirection(4)).toBe("forward");
  });

  test("leaves the buttons that open documents alone", () => {
    // Left and middle belong to the markdown link handler; right opens the
    // context menu.
    expect(navigationDirection(0)).toBeNull();
    expect(navigationDirection(1)).toBeNull();
    expect(navigationDirection(2)).toBeNull();
    expect(navigationDirection(5)).toBeNull();
  });
});

describe("the side-button listener", () => {
  let directions: string[];

  beforeEach(() => {
    directions = [];
    // The listener is the same function every time, so the repeated call
    // across tests registers it once.
    setup();
    onNavigate((direction) => directions.push(direction));
  });

  /** A press of `button` on the document, and whether the page kept it. */
  const press = (button: number): boolean => {
    const event = new MouseEvent("mousedown", { button, bubbles: true, cancelable: true });
    document.body.dispatchEvent(event);
    return event.defaultPrevented;
  };

  test("hands a side-button press to the callback", () => {
    press(3);
    press(4);
    expect(directions).toEqual(["back", "forward"]);
  });

  test("keeps the WebView from walking its own history", () => {
    expect(press(3)).toBe(true);
  });

  test("lets every other button through untouched", () => {
    expect(press(0)).toBe(false);
    expect(directions).toEqual([]);
  });
});
