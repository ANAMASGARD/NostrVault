import { expect, it } from "vitest";
import { searchMessages, visibleText } from "./viewer";
import type { LogicalMessage } from "./chats";

const message = (body: string, visible = true): LogicalMessage => ({
  rumorId: body,
  outerIds: ["w"],
  sender: "a",
  createdAt: 1,
  participants: ["a", "b"],
  body,
  state: "verified",
  visible,
});

it("searches visible text only and strips hostile markup", () => {
  expect(visibleText('<img src=x onerror="alert(1)"><b>Hello</b>')).toBe(
    "Hello",
  );
  const hits = searchMessages(
    [
      message("Hello vault"),
      message("secret", false),
      message("<script>x</script>note"),
    ],
    "hello",
  );
  expect(hits.map((item) => item.body)).toEqual(["Hello vault"]);
  expect(searchMessages([message("Hello vault")], "   ")).toHaveLength(1);
});
