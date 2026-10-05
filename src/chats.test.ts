import { expect, it } from "vitest";
import { conversationViews, projectMessages } from "./chats";

it("collapses wraps and keeps pending, failed, and verified distinct", () => {
  const messages = projectMessages([
    {
      rumorId: "r1",
      outerId: "w2",
      sender: "a",
      claimedSender: "a",
      createdAt: 10,
      participants: ["b", "a"],
      body: "hello",
      deleted: false,
      expired: false,
      state: "verified",
    },
    {
      rumorId: "r1",
      outerId: "w1",
      sender: "a",
      claimedSender: "a",
      createdAt: 10,
      participants: ["a", "b"],
      body: "hello",
      deleted: false,
      expired: false,
      state: "pending",
    },
    {
      rumorId: "r2",
      outerId: "w3",
      sender: "a",
      claimedSender: "a",
      createdAt: 11,
      participants: ["a", "b"],
      body: "nope",
      deleted: false,
      expired: false,
      state: "failed",
    },
  ]);
  expect(messages).toHaveLength(2);
  expect(messages[0]?.outerIds).toEqual(["w1", "w2"]);
  expect(messages[0]?.state).toBe("verified");
  expect(messages[1]?.body).toBe("");
  expect(messages[1]?.state).toBe("failed");
  expect(() =>
    projectMessages([
      {
        rumorId: "r",
        outerId: "w",
        sender: "a",
        claimedSender: "b",
        createdAt: 1,
        participants: ["a"],
        body: "x",
        state: "verified",
      },
    ]),
  ).toThrow();
  const views = conversationViews(messages, { a: "Ada", b: "Bea" });
  expect(views).toHaveLength(1);
  expect(views[0]?.label).toBe("Ada, Bea");
});
