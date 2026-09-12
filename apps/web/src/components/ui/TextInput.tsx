import type { InputHTMLAttributes } from "react";
import { controlBorder, controlClass } from "@/components/ui/Field";

type Props = InputHTMLAttributes<HTMLInputElement> & {
  id: string;
  hasError?: boolean;
  hasHint?: boolean;
};

export function TextInput({ id, hasError = false, hasHint = false, className = "", ...rest }: Props) {
  return (
    <input
      id={id}
      name={id}
      aria-invalid={hasError || undefined}
      aria-describedby={hasError ? `${id}-error` : hasHint ? `${id}-hint` : undefined}
      className={`${controlClass} ${controlBorder(hasError)} ${className}`}
      {...rest}
    />
  );
}
