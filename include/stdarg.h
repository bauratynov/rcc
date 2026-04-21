#ifndef _STDARG_H
#define _STDARG_H

typedef char *va_list;
#define va_start(ap, param) (ap = (va_list)&param + 8)
#define va_end(ap) ((void)0)
#define va_arg(ap, type) (*(type *)((ap += 8) - 8))
#define __va_copy(dest, src) (dest = src)
#define va_copy(dest, src) (dest = src)

#endif
