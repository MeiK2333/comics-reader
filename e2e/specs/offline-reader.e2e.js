describe("offline reading", () => {
  it("falls back to the downloaded comic on the detail page", async () => {
    await browser.execute(() => {
      location.hash = "#/comic/nhentai/900001";
    });

    const offlineSummary = await $(".offline-summary");
    await offlineSummary.waitForExist({ timeout: 25_000 });

    expect(await $(".detail-page").getText()).toContain(
      "E2E Offline Gallery",
    );
    expect(await $(".offline-notice").isExisting()).toBe(true);
  });

  it("renders downloaded pages and turns a page with the keyboard", async () => {
    await browser.execute(() => {
      location.hash = "#/comic/nhentai/900001/read";
    });

    const viewer = await $(".viewer");
    await viewer.waitForExist({ timeout: 25_000 });

    await browser.waitUntil(async () => (await $$(".viewer img")).length > 0);
    const images = await $$(".viewer img");
    expect(await images[0].getAttribute("src")).toBeTruthy();

    expect(await $(".viewer__hud-page").getText()).toBe("1 / 2");

    await browser.keys(["ArrowRight"]);

    await browser.waitUntil(
      async () => (await $(".viewer__hud-page").getText()) === "2 / 2",
      { timeoutMsg: "ArrowRight should advance to the second page" },
    );
    await browser.waitUntil(
      async () => (await browser.execute(() => location.hash)).includes("page=2"),
      { timeoutMsg: "the reader should mirror the page in the hash query" },
    );
  });
});
