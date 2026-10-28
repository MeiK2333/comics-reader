import { definePlatform } from "../types";
import icon from "./icon.png";
import { copymangaMessages } from "./messages";

export const copymanga = definePlatform<"copymanga">({
  id: "copymanga",
  messages: copymangaMessages,
  icon,
  iconStyle: "plain",
  singleChapter: false,
  numericIdDirectOpen: false,
});
