describe("settings persistence", () => {
  it("rehydrates theme, locale and reader preferences on reload", async () => {
    await browser.execute(
      (settings) => {
        localStorage.setItem("comics-reader:settings", JSON.stringify(settings));
      },
      {
        theme: "dark",
        locale: "en",
        proxy: { enabled: false, host: "", port: 8080 },
        ech: false,
        logging: false,
        readerDirection: "rtl",
        readerPreloadCount: 7,
        readerPreloadNextChapter: false,
      },
    );

    await browser.execute(() => location.reload()).catch(() => undefined);

    await browser.waitUntil(
      async () =>
        (await browser.execute(
          () => document.documentElement.dataset.theme,
        )) === "dark",
      { timeoutMsg: "the persisted dark theme should be applied on startup" },
    );

    expect(await browser.execute(() => document.documentElement.lang)).toBe(
      "en",
    );

    await browser.execute(() => {
      location.hash = "#/settings";
    });
    await $("main.settings-page").waitForExist();

    const persisted = await browser.execute(() =>
      JSON.parse(localStorage.getItem("comics-reader:settings") ?? "{}"),
    );
    expect(persisted.readerPreloadCount).toBe(7);
    expect(persisted.readerDirection).toBe("rtl");
    expect(persisted.readerPreloadNextChapter).toBe(false);
  });
});
