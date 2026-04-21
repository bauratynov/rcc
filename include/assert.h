#ifndef _ASSERT_H
#define _ASSERT_H

void abort(void);

#ifdef NDEBUG
#define assert(x) ((void)0)
#else
#define assert(x) ((void)0)
#endif

#endif
