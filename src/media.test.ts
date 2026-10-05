import { expect, it } from "vitest";
import { mediaLabel } from "./media";

it("does not call a URL a backed-up file", () => {
  expect(mediaLabel("missing")).toContain("not in this vault");
  expect(mediaLabel("rejected")).toContain("rejected");
  expect(mediaLabel("stored")).toContain("offline preview");
});
