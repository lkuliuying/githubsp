// 改编自 cc-switch（MIT）；来源与版权见 desktop/THIRD_PARTY_NOTICES.md。
import * as React from "react";
import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { cn } from "@/lib/utils";

const buttonVariants = cva("button", {
  variants: {
    variant: {
      default: "button-primary",
      outline: "button-outline",
      ghost: "button-ghost",
      destructive: "button-danger",
    },
    size: { default: "", sm: "button-small", icon: "button-icon" },
  },
  defaultVariants: { variant: "outline", size: "default" },
});
export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
  /** 悬浮提示文字；纯图标按钮默认使用 aria-label，传 false 关闭。 */
  tooltip?: string | false;
}
export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  (
    {
      className,
      variant,
      size,
      asChild = false,
      type = "button",
      tooltip,
      ...props
    },
    ref,
  ) => {
    const Comp = asChild ? Slot : "button";
    const tip =
      tooltip === false
        ? undefined
        : (tooltip ?? (size === "icon" ? props["aria-label"] : undefined));
    return (
      <Comp
        type={type}
        ref={ref}
        className={cn(buttonVariants({ variant, size, className }))}
        data-tooltip={tip || undefined}
        {...props}
      />
    );
  },
);
Button.displayName = "Button";
