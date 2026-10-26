describe("favorites", () => {
  it("lists seeded favorites and filters by group", async () => {
    await browser.execute(() => {
      location.hash = "#/favorites";
    });

    await browser.waitUntil(
      async () => (await $$(".favorite-entry")).length === 2,
      { timeoutMsg: "both seeded favorites should render" },
    );

    const list = await $(".favorites-list");
    expect(await list.getText()).toContain("E2E Favorite One");
    expect(await list.getText()).toContain("E2E Favorite Two");

    const group = await $("button*=E2E Group");
    await group.click();

    await browser.waitUntil(
      async () => (await $$(".favorite-entry")).length === 1,
      { timeoutMsg: "the group filter should narrow the list" },
    );

    const filtered = await $(".favorites-list");
    expect(await filtered.getText()).toContain("E2E Favorite Two");
    expect(await filtered.getText()).not.toContain("E2E Favorite One");
  });
});
