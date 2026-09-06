/**
 * @vitest-environment jsdom
 */
import { cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import "@/test/jsdomStubs";
import { HtmlBrowser } from "./HtmlBrowser";

afterEach(cleanup);

describe("HtmlBrowser", () => {
  it("renders iframe with sandbox='allow-scripts' and without allow-same-origin", () => {
    const { container } = render(
      <HtmlBrowser title="Preview" html="<h1>Hello sandboxed world</h1>" />,
    );

    const iframe = container.querySelector("iframe");
    expect(iframe).not.toBeNull();

    const sandbox = iframe?.getAttribute("sandbox");
    expect(sandbox).toBe("allow-scripts");
    expect(sandbox).not.toContain("allow-same-origin");
  });
});
