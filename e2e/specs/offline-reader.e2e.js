describe("offline reading", () => {
  let gestureSequence = 0;

  function nextGestureId() {
    gestureSequence += 1;
    return `reader-finger-${gestureSequence}`;
  }

  async function readerPage() {
    return (await $(".viewer__hud-page").getText()).trim();
  }

  async function waitForReaderPage(label) {
    await browser.waitUntil(async () => (await readerPage()) === label, {
      timeoutMsg: `reader should show ${label}`,
    });
  }

  async function waitForTurnSettled() {
    await browser.waitUntil(
      async () =>
        browser.execute(() => {
          const track = document.querySelector(".viewer__track");
          if (!track) {
            return true;
          }
          const style = getComputedStyle(track);
          return (
            style.transitionProperty === "none" ||
            style.transitionDuration === "0s"
          );
        }),
      { timeoutMsg: "the page-turn animation should settle" },
    );
  }

  async function swipe(stageRect, fromRatio, toRatio) {
    const y = Math.round(stageRect.y + stageRect.height / 2);
    const fromX = Math.round(stageRect.x + stageRect.width * fromRatio);
    const toX = Math.round(stageRect.x + stageRect.width * toRatio);

    await browser.performActions([
      {
        type: "pointer",
        id: nextGestureId(),
        parameters: { pointerType: "touch" },
        actions: [
          {
            type: "pointerMove",
            duration: 0,
            origin: "viewport",
            x: fromX,
            y,
          },
          { type: "pointerDown", button: 0 },
          { type: "pointerMove", duration: 160, origin: "viewport", x: toX, y },
          { type: "pointerUp", button: 0 },
        ],
      },
    ]);
    await browser.releaseActions();
  }

  async function tap(stageRect, xRatio) {
    const x = Math.round(stageRect.x + stageRect.width * xRatio);
    const y = Math.round(stageRect.y + stageRect.height / 2);

    await browser.performActions([
      {
        type: "pointer",
        id: nextGestureId(),
        parameters: { pointerType: "touch" },
        actions: [
          { type: "pointerMove", duration: 0, origin: "viewport", x, y },
          { type: "pointerDown", button: 0 },
          { type: "pointerUp", button: 0 },
        ],
      },
    ]);
    await browser.releaseActions();
  }

  async function openReader() {
    await browser.execute(() => {
      // Force a query change so the reader resets even when the previous spec
      // left the same chapter open on page 2.
      location.hash = "#/comic/nhentai/900001/read?page=1";
    });
    const viewer = await $(".viewer");
    await viewer.waitForExist({ timeout: 25_000 });
    await waitForReaderPage("1 / 2");
  }

  async function readerStageRect() {
    return browser.execute(() => {
      const element = document.querySelector(".viewer__stage");
      if (!element) {
        return null;
      }
      const rect = element.getBoundingClientRect();
      return {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
      };
    });
  }

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

    await waitForReaderPage("1 / 2");

    await browser.keys(["ArrowRight"]);

    await waitForReaderPage("2 / 2");
    await browser.waitUntil(
      async () => (await browser.execute(() => location.hash)).includes("page=2"),
      { timeoutMsg: "the reader should mirror the page in the hash query" },
    );
  });

  it("turns pages with horizontal swipes and tap zones", async () => {
    await openReader();
    const rect = await readerStageRect();
    expect(rect).not.toBeNull();

    await swipe(rect, 0.75, 0.25);
    await waitForReaderPage("2 / 2");
    await waitForTurnSettled();

    await swipe(rect, 0.25, 0.75);
    await waitForReaderPage("1 / 2");
    await waitForTurnSettled();

    await tap(rect, 0.85);
    await waitForReaderPage("2 / 2");
    await waitForTurnSettled();

    await tap(rect, 0.15);
    await waitForReaderPage("1 / 2");
    await waitForTurnSettled();

    expect(
      await browser.execute(() =>
        document
          .querySelector(".viewer__bar--top")
          ?.classList.contains("viewer__bar--hidden"),
      ),
    ).toBe(true);

    await tap(rect, 0.5);
    await browser.waitUntil(
      async () =>
        (await browser.execute(() =>
          document
            .querySelector(".viewer__bar--top")
            ?.classList.contains("viewer__bar--hidden"),
        )) === false,
      { timeoutMsg: "tapping the middle third should show the chrome" },
    );
    expect(await $(".viewer__hud").isDisplayed()).toBe(false);
  });
});
