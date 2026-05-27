import { ChevronLeft, ChevronRight } from "lucide-react";
import {
  DayPicker,
  DayFlag,
  SelectionState,
  UI,
  getDefaultClassNames,
  type DayPickerProps,
} from "react-day-picker";

import { cn } from "@/lib/utils";

function Calendar({
  className,
  classNames,
  showOutsideDays = true,
  ...props
}: DayPickerProps & { className?: string }) {
  const defaultClassNames = getDefaultClassNames();

  return (
    <DayPicker
      showOutsideDays={showOutsideDays}
      className={cn("shadcn-calendar", className)}
      classNames={{
        ...defaultClassNames,
        [UI.Root]: cn("shadcn-calendar-root", defaultClassNames[UI.Root]),
        [UI.Months]: cn("shadcn-calendar-months", defaultClassNames[UI.Months]),
        [UI.Month]: cn("shadcn-calendar-month", defaultClassNames[UI.Month]),
        [UI.MonthCaption]: cn("shadcn-calendar-caption", defaultClassNames[UI.MonthCaption]),
        [UI.CaptionLabel]: cn("shadcn-calendar-caption-label", defaultClassNames[UI.CaptionLabel]),
        [UI.Nav]: cn("shadcn-calendar-nav", defaultClassNames[UI.Nav]),
        [UI.PreviousMonthButton]: cn("shadcn-calendar-nav-button", defaultClassNames[UI.PreviousMonthButton]),
        [UI.NextMonthButton]: cn("shadcn-calendar-nav-button", defaultClassNames[UI.NextMonthButton]),
        [UI.MonthGrid]: cn("shadcn-calendar-grid", defaultClassNames[UI.MonthGrid]),
        [UI.Weekdays]: cn("shadcn-calendar-weekdays", defaultClassNames[UI.Weekdays]),
        [UI.Weekday]: cn("shadcn-calendar-weekday", defaultClassNames[UI.Weekday]),
        [UI.Week]: cn("shadcn-calendar-week", defaultClassNames[UI.Week]),
        [UI.Day]: cn("shadcn-calendar-day", defaultClassNames[UI.Day]),
        [UI.DayButton]: cn("shadcn-calendar-day-button", defaultClassNames[UI.DayButton]),
        [DayFlag.outside]: cn("shadcn-calendar-outside", defaultClassNames[DayFlag.outside]),
        [DayFlag.disabled]: cn("shadcn-calendar-disabled", defaultClassNames[DayFlag.disabled]),
        [SelectionState.selected]: cn("shadcn-calendar-selected", defaultClassNames[SelectionState.selected]),
        ...classNames,
      }}
      components={{
        Chevron: ({ orientation, className: chevronClassName, ...chevronProps }) => (
          orientation === "left"
            ? <ChevronLeft className={cn("size-4", chevronClassName)} {...chevronProps} />
            : <ChevronRight className={cn("size-4", chevronClassName)} {...chevronProps} />
        ),
        ...props.components,
      }}
      {...props}
    />
  );
}

export { Calendar };
