// 改编自 cc-switch（MIT）；保留 ref 转发与原生输入语义。
import * as React from "react";
import { cn } from "@/lib/utils";
export const Input = React.forwardRef<
  HTMLInputElement,
  React.InputHTMLAttributes<HTMLInputElement>
>(({ className, ...props }, ref) => (
  <input
    ref={ref}
    className={cn("input", className)}
    autoComplete="off"
    spellCheck={false}
    {...props}
  />
));
Input.displayName = "Input";
