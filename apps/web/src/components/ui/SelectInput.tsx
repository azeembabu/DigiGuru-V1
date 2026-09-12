import type { SelectHTMLAttributes } from "react";
import { controlBorder, controlClass } from "@/components/ui/Field";

type Option = { id: string; name: string };

type Props = Omit<SelectHTMLAttributes<HTMLSelectElement>, "children"> & {
  id: string;
  options: Option[];
  placeholder: string;
  hasError?: boolean;
  hasHint?: boolean;
};

export function SelectInput({
  id,
  options,
  placeholder,
  hasError = false,
  hasHint = false,
  className = "",
  ...rest
}: Props) {
  return (
    <select
      id={id}
      name={id}
      aria-invalid={hasError || undefined}
      aria-describedby={hasError ? `${id}-error` : hasHint ? `${id}-hint` : undefined}
      // `color-scheme: dark` makes the native option list render dark; without
      // it the popup inherits the OS light palette and is unreadable here.
      style={{ colorScheme: "dark" }}
      className={`${controlClass} ${controlBorder(hasError)} ${className}`}
      {...rest}
    >
      <option value="">{placeholder}</option>
      {options.map((option) => (
        <option key={option.id} value={option.id}>
          {option.name}
        </option>
      ))}
    </select>
  );
}
