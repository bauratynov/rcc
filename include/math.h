#ifndef _MATH_H
#define _MATH_H

double floor(double x);
double ceil(double x);
double sqrt(double x);
double pow(double x, double y);
double fabs(double x);
double log(double x);
double log10(double x);
double exp(double x);
double sin(double x);
double cos(double x);
double tan(double x);
int isnan(double x);
int isinf(double x);

#define HUGE_VAL 1e308
#define NAN (0.0/0.0)
#define INFINITY HUGE_VAL

#endif
