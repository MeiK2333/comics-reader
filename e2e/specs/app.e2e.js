describe("application shell", () => {
  before(async () => {
    await $("main.home").waitForExist();
    // The WebView profile survives runs; start every suite from a clean
    // settings preference so the persisted-state assertions are meaningful.
    await browser.execute(() => localStorage.clear());
    await browser.execute(() => location.reload()).catch(() => undefined);
    await $("main.home").waitForExist();
  });

  it("opens the home page with the localized title and shortcuts", async () => {
    await browser.waitUntil(
      async () => (await browser.getTitle()).includes("Comics Reader"),
      { timeoutMsg: "the window title should name the app" },
    );

    const home = await $("main.home");
    await home.waitForExist();

    const links = await $$("main.home a");
    const hrefs = [];
    for (const link of links) {
      hrefs.push(await link.getAttribute("href"));
    }
    expect(hrefs).toEqual(
      expect.arrayContaining([
        "#/favorites",
        "#/downloads",
        "#/history",
        "#/settings",
      ]),
    );
  });
});
